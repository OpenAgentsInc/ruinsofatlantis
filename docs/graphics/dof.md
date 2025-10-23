Great—what you’re asking for is “phone-as-head” tracking: move/rotate the device and the camera in your Rust/wgpu world follows. There are two clean ways to do it on iOS:

* **3‑DoF head tracking (Core Motion):** orientation only (yaw/pitch/roll). Easiest, no camera permission, tiny battery hit.
* **6‑DoF AR head tracking (ARKit):** orientation **+ position** in real world, derived from the rear camera + IMU. Heavier, needs camera permission, but rock‑solid yaw (little drift) and you can actually walk around.

Below I show both. They plug straight into the Swift‑host + Rust‑engine structure we set up.

---

## 0) The data you’ll pass into Rust each frame

Two minimal FFI shapes cover everything:

```c
// C header you expose from Rust with cbindgen (names are examples)
typedef struct {
    float qw, qx, qy, qz;    // device/camera orientation as a unit quaternion (world <- camera)
    double t_seconds;        // a timestamp; pass CACurrentMediaTime() or ARFrame.timestamp
} HeadOrientation3DoF;

void wgpu_renderer_set_head_orientation(void* handle, HeadOrientation3DoF pose);

// If using ARKit 6DoF, just hand Rust the ready-to-use camera matrices:
typedef struct {
    float view[16];          // column-major world->camera
    float proj[16];          // column-major camera->clip (Metal/WebGPU depth: 0..1)
    double t_seconds;
} CameraMatrices;

void wgpu_renderer_set_camera_matrices(void* handle, CameraMatrices cam);
```

> You’ll call **one** of these per frame from Swift, right before you call `wgpu_renderer_render(...)`.

---

## 1) Option A — 3‑DoF tracking with Core Motion (no camera)

**What you get:** yaw/pitch/roll from the gyro, stabilized by gravity. This is enough to “look around” a world from a fixed or engine‑controlled position.

### Swift: read device orientation and send it to Rust

```swift
import CoreMotion
import simd

final class HeadTracker3DoF {
    private let mm = CMMotionManager()
    private let queue = OperationQueue()
    private var qZero: simd_quatf?        // for "tap to recenter" offset
    private(set) var latest: simd_quatf = simd_quatf(ix: 0, iy: 0, iz: 0, r: 1)

    func start() {
        guard mm.isDeviceMotionAvailable else { return }
        mm.deviceMotionUpdateInterval = 1.0 / 120.0 // ProMotion-friendly
        // Z is global up; yaw is free (we'll zero it ourselves)
        mm.startDeviceMotionUpdates(using: .xArbitraryZVertical, to: queue) { [weak self] motion, _ in
            guard let self, let m = motion else { return }
            // Core Motion gives quaternion as (x, y, z, w) for world->device
            let q_w2d = simd_quatf(ix: Float(m.attitude.quaternion.x),
                                   iy: Float(m.attitude.quaternion.y),
                                   iz: Float(m.attitude.quaternion.z),
                                   r:  Float(m.attitude.quaternion.w))
            // We want camera (device) orientation in WORLD: device->world = inverse(world->device)
            var q_d2w = q_w2d.inverse

            // Optional: on first run or when user taps "recenter", define qZero so current yaw becomes 0
            if self.qZero == nil { self.qZero = q_d2w }
            if let q0 = self.qZero {
                // Apply yaw-only zeroing: remove the initial yaw component so forward matches your game’s "forward"
                let yawPitchRoll = self.ypr(from: q0)
                let qRemoveYaw = simd_quatf(angle: -yawPitchRoll.yaw, axis: simd_float3(0,1,0))
                q_d2w = qRemoveYaw * q_d2w
            }

            // IMPORTANT: phone’s “look vector” is out through the BACK of the phone → device -Z.
            // The q_d2w we computed already maps device axes to world. No extra handness fix needed if you
            // build your view matrix from this quaternion in Rust the right way (see Rust section).
            self.latest = q_d2w
        }
    }

    func recenterYaw() { qZero = nil } // call this on a button/tap gesture

    private func ypr(from q: simd_quatf) -> (yaw: Float, pitch: Float, roll: Float) {
        // Y-up convention, yaw around +Y. Matches most game cameras.
        let m = float3x3(q)
        let pitch = asin(-m.columns.2.y)
        let yaw   = atan2(m.columns.2.x, m.columns.2.z)
        let roll  = atan2(m.columns.1.y, m.columns.0.y)
        return (yaw, pitch, roll)
    }
}
```

**Drive it from your `MTKView` draw loop:**

```swift
func draw(in view: MTKView) {
    // ... your usual rendering tick
    let q = headTracker.latest
    var pose = HeadOrientation3DoF(qw: q.real,
                                   qx: q.imag.x, qy: q.imag.y, qz: q.imag.z,
                                   t_seconds: CACurrentMediaTime())
    wgpu_renderer_set_head_orientation(handle, pose)
    wgpu_renderer_render(handle, CACurrentMediaTime())
}
```

### Rust: turn quaternion into a view matrix and sync your “oscillator”

In your renderer, keep a camera uniform:

```rust
use glam::{Mat4, Vec3, Quat, EulerRot};

pub struct CameraState {
    pub pos: Vec3,             // your player position in world units
    pub ori: Quat,             // camera orientation (device->world)
    pub proj: Mat4,            // your perspective projection (0..1 depth for wgpu)
}

// For OpenGL-style proj (−1..1 depth) multiply by this to get WebGPU 0..1 depth:
const OPENGL_TO_WGPU: Mat4 = Mat4::from_cols_array(&[
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
]);

impl CameraState {
    pub fn view(&self) -> Mat4 {
        // self.ori is device->world; the view needs world->camera:
        let r_view = self.ori.inverse().to_mat4();
        r_view * Mat4::from_translation(-self.pos)
    }
}
```

**FFI setter**:

```rust
#[repr(C)]
pub struct HeadOrientation3DoF { pub qw:f32, pub qx:f32, pub qy:f32, pub qz:f32, pub t_seconds:f64 }

#[no_mangle]
pub extern "C" fn wgpu_renderer_set_head_orientation(ptr: *mut Renderer, pose: HeadOrientation3DoF) {
    let r = unsafe { &mut *ptr };
    let q = Quat::from_xyzw(pose.qx, pose.qy, pose.qz, pose.qw); // note xyzw order
    r.camera.ori = q.normalize();

    // Example “oscillator sync”: drive audio params from yaw/pitch rates.
    // Estimate angular velocity using consecutive quats (cheap & stable for small dt).
    let dt = (pose.t_seconds - r.last_pose_time).max(1.0/240.0) as f32;
    let dq = r.last_quat.conjugate() * r.camera.ori;
    let axis = Vec3::new(dq.x, dq.y, dq.z);
    let angle = 2.0 * axis.length().atan2(dq.w);   // signed angle
    let omega = if angle.abs() < 1e-5 { Vec3::ZERO } else { axis.normalize() * (angle/dt) };

    // Map yaw rate to frequency/pan, pitch to filter cutoff, etc. Your call:
    let (_, pitch, _) = r.camera.ori.to_euler(EulerRot::YXZ); // yaw/pitch/roll if needed
    r.audio.set_osc_params(/*freq:*/ 220.0 + 20.0*omega.y,
                           /*pan: */ (omega.y*0.1).clamp(-1.0, 1.0),
                           /*tilt:*/ pitch);

    r.last_quat = r.camera.ori;
    r.last_pose_time = pose.t_seconds;
}
```

> In your per‑frame uniform update, use `view * proj` and upload to wgpu. For Core Motion you’ll likely build `proj = OPENGL_TO_WGPU * Mat4::perspective_rh(fov_y, aspect, znear, zfar)` (or directly make a Metal/WebGPU‑style 0..1 projection if you have a helper).

**Why this works:** Core Motion’s quaternion gives you the device’s pose; treating the phone as the camera, its **forward** axis is device **−Z** (out through the back of the phone), and the “view” matrix is just the inverse of the camera orientation (plus your translation).

---

## 2) Option B — 6‑DoF tracking with ARKit (camera on, walk around)

**What you get:** position + orientation locked to the real world. Perfect “first person” feeling; you can pace around, and your virtual camera moves accordingly.

### Swift: set up an `ARSession` and feed its camera matrices

```swift
import ARKit
import simd

final class ARHeadTracker: NSObject, ARSessionDelegate {
    let session = ARSession()
    var latestView = matrix_identity_float4x4
    var latestProj = matrix_identity_float4x4
    var latestT: CFTimeInterval = 0

    func start(viewportSize: CGSize, orientation: UIInterfaceOrientation) {
        guard ARWorldTrackingConfiguration.isSupported else { return }
        let config = ARWorldTrackingConfiguration()
        config.worldAlignment = .gravity // or .gravityAndHeading if you want north alignment
        config.frameSemantics = []       // add .sceneDepth if you need depth
        session.delegate = self
        session.run(config, options: [.resetTracking, .removeExistingAnchors])
    }

    func session(_ session: ARSession, didUpdate frame: ARFrame) {
        let cam = frame.camera
        // ARKit gives camera in world. View = inverse(transform).
        latestView = simd_inverse(cam.transform)

        // Projection with Metal/WebGPU-friendly depth 0..1:
        let drawable = UIScreen.main.nativeBounds.size // or MTKView.drawableSize
        latestProj = cam.projectionMatrix(for: .landscapeRight,
                                          viewportSize: CGSize(width: drawable.width, height: drawable.height),
                                          zNear: 0.01, zFar: 100.0)
        latestT = frame.timestamp
    }
}
```

**Draw loop:** hand those matrices to Rust before you render.

```swift
func draw(in view: MTKView) {
    var cam = CameraMatrices(view: latestView.arrayColumnMajor(),  // helper that flattens simd_float4x4
                             proj: latestProj.arrayColumnMajor(),
                             t_seconds: latestT > 0 ? latestT : CACurrentMediaTime())
    wgpu_renderer_set_camera_matrices(handle, cam)
    wgpu_renderer_render(handle, CACurrentMediaTime())
}

private extension simd_float4x4 {
    func arrayColumnMajor() -> [Float] {
        var out = [Float](repeating: 0, count: 16)
        // simd matrices are column-major already
        withUnsafeBytes(of: self) { raw in
            out.withUnsafeMutableBytes { dst in dst.copyMemory(from: raw) }
        }
        return out
    }
}
```

> **Optional**: Show the real camera feed behind your rendering. Easiest way without touching wgpu textures is to insert an `ARSCNView`/`ARView` or `AVCaptureVideoPreviewLayer` **below** your `MTKView`. If you *do* want to sample the camera in your Rust shaders, convert `ARFrame.capturedImage` (CVPixelBuffer) → `MTLTexture` with a `CVMetalTextureCache` on the Swift side and pass that into a small Metal pre‑pass, or keep the composition on the Swift side.

### Rust: consume ready‑made matrices

```rust
#[repr(C)]
pub struct CameraMatrices { pub view:[f32;16], pub proj:[f32;16], pub t_seconds:f64 }

#[no_mangle]
pub extern "C" fn wgpu_renderer_set_camera_matrices(ptr:*mut Renderer, cam: CameraMatrices) {
    let r = unsafe { &mut *ptr };
    let view = Mat4::from_cols_array(&cam.view);
    let proj = Mat4::from_cols_array(&cam.proj);
    r.camera_from_arkit(view, proj, cam.t_seconds);
}

// ...in your renderer:
pub fn camera_from_arkit(&mut self, view: Mat4, proj: Mat4, t: f64) {
    self.camera_view = view;
    self.camera_proj = proj; // ARKit already uses 0..1 depth like Metal/WebGPU
    // Drive oscillator or gameplay with t or derived velocities if desired
}
```

That’s it—ARKit does the hard work: drift‑free yaw, real‑space meters, and a projection matched to the device optics and screen orientation.

---

## 3) Wiring into your player controller & “oscillator”

Whether you choose Core Motion or ARKit, do this in your engine tick:

1. **Build the view matrix** (as shown above).
2. **Extract yaw/pitch/roll or angular velocity** if you want to couple audio/gameplay:

   ```rust
   let (yaw, pitch, _roll) = camera_quat.to_euler(glam::EulerRot::YXZ);
   audio.osc.frequency = 220.0 + 80.0 * yaw.sin();   // silly example
   audio.osc.cutoff    = 800.0 + 600.0 * pitch.cos();
   ```
3. **Smoothing:** IMU noise benefits from a small one‑pole filter:

   ```rust
   fn smooth(prev: f32, next: f32, alpha: f32) -> f32 { prev + alpha*(next - prev) }
   // alpha ≈ 0.15 at 60 Hz feels responsive but stable
   ```
4. **Recenter:** map a gesture to zero yaw (Core Motion) or reset tracking (ARKit) when the player wants to “face forward”.

---

## 4) Coordinate frames & gotchas (so you don’t fight axes for a day)

* **Device forward** is **−Z** (into the screen/back of the phone).
* **View = inverse(camera transform).** If you hold a quaternion that says “camera (device) in world”, invert it to get world→camera.
* **Depth range:** wgpu/WebGPU/Metal use **0..1**. If you build your own projection (Core Motion path), multiply an OpenGL‑style perspective by the `OPENGL_TO_WGPU` matrix shown above.
* **Screen orientation:** With ARKit, pass the current UI orientation into `projectionMatrix(...)`. With Core Motion, orientation doesn’t matter—you already invert world→device and then recenter the yaw.
* **Drift:** Core Motion yaw drifts over minutes. Give the player a **recenter** action or use `.xMagneticNorthZVertical` (adds compass noise). ARKit basically solves drift.
* **Units:** ARKit world units are **meters**. Decide your engine’s “1 unit = 1 meter” or scale accordingly.

---

## 5) Minimal checklists

**Core Motion (3‑DoF)**

* [ ] Start `CMMotionManager` with `.xArbitraryZVertical`
* [ ] In each draw: read quaternion → send to Rust via `wgpu_renderer_set_head_orientation`
* [ ] In Rust: `view = quat.inverse * translate(-pos)`; build `proj` for wgpu; update UBO
* [ ] Optional: recenter yaw on tap; smooth orientation a bit; map angles to audio

**ARKit (6‑DoF)**

* [ ] Start `ARSession` with `ARWorldTrackingConfiguration`
* [ ] On each frame: `view = inverse(camera.transform)`, `proj = camera.projectionMatrix(...)`
* [ ] Send matrices with `wgpu_renderer_set_camera_matrices`
* [ ] Optional: background camera preview under your `MTKView`; anchor virtual objects in meters

---

## 6) A tiny, end‑to‑end example you can drop in

**Swift (Core Motion path in your `MTKView` delegate):**

```swift
let head = HeadTracker3DoF()
head.start()

func draw(in view: MTKView) {
    let q = head.latest
    var pose = HeadOrientation3DoF(qw: q.real, qx: q.imag.x, qy: q.imag.y, qz: q.imag.z,
                                   t_seconds: CACurrentMediaTime())
    wgpu_renderer_set_head_orientation(handle, pose)
    wgpu_renderer_render(handle, pose.t_seconds)
}
```

**Rust (camera UBO update before encoding passes):**

```rust
// build view-proj and upload
let view = renderer.camera.view();
let proj = renderer.camera.proj;
let vp   = proj * view;               // column-major (Mat4) for shaders
queue.write_buffer(&camera_buffer, 0, bytemuck::bytes_of(&vp));
```

GLSL/WGSL (vertex):

```wgsl
struct Camera { view_proj: mat4x4<f32>; };
@group(0) @binding(0) var<uniform> camera: Camera;

@vertex
fn vs_main(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
    return camera.view_proj * vec4<f32>(pos, 1.0);
}
```

---

## 7) Which should you pick?

* If you just need “look around” and your game’s movement is controlled by your own inputs → **Core Motion** (fastest path, no camera).
* If you want true “move your head/body through space” or iron‑clad yaw → **ARKit**.

Either way, the Swift host collects the pose and your Rust + wgpu renderer simply treats it as the camera and (optionally) also uses it to drive your oscillator.

If you want, I can tailor the exact FFI header and the Rust camera math to your engine’s conventions (handedness, up axis, your existing math lib), or extend this with an AR background pass so your world composites over the live camera.
