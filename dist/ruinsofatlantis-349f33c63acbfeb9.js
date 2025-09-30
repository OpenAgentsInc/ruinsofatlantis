let wasm;

function isLikeNone(x) {
    return x === undefined || x === null;
}

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_export_1.set(idx, obj);
    return idx;
}

let cachedUint8ArrayMemory0 = null;

function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });

cachedTextDecoder.decode();

const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

let WASM_VECTOR_LEN = 0;

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    }
}

function passStringToWasm0(arg, malloc, realloc) {

    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }

    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

let cachedDataViewMemory0 = null;

function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedUint32ArrayMemory0 = null;

function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(
state => {
    wasm.__wbindgen_export_6.get(state.dtor)(state.a, state.b);
}
);

function makeMutClosure(arg0, arg1, dtor, f) {
    const state = { a: arg0, b: arg1, cnt: 1, dtor };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            if (--state.cnt === 0) {
                wasm.__wbindgen_export_6.get(state.dtor)(a, state.b);
                CLOSURE_DTORS.unregister(state);
            } else {
                state.a = a;
            }
        }
    };
    real.original = state;
    CLOSURE_DTORS.register(real, state, state);
    return real;
}
function __wbg_adapter_6(arg0, arg1, arg2) {
    wasm.closure575_externref_shim(arg0, arg1, arg2);
}

function __wbg_adapter_9(arg0, arg1, arg2) {
    wasm.closure990_externref_shim(arg0, arg1, arg2);
}

function __wbg_adapter_12(arg0, arg1, arg2, arg3) {
    wasm.closure573_externref_shim(arg0, arg1, arg2, arg3);
}

function __wbg_adapter_31(arg0, arg1) {
    wasm.wasm_bindgen__convert__closures_____invoke__h25302bc53713aef5(arg0, arg1);
}

function __wbg_adapter_34(arg0, arg1, arg2) {
    wasm.closure1007_externref_shim(arg0, arg1, arg2);
}

const __wbindgen_enum_GpuAddressMode = ["clamp-to-edge", "repeat", "mirror-repeat"];

const __wbindgen_enum_GpuBlendFactor = ["zero", "one", "src", "one-minus-src", "src-alpha", "one-minus-src-alpha", "dst", "one-minus-dst", "dst-alpha", "one-minus-dst-alpha", "src-alpha-saturated", "constant", "one-minus-constant", "src1", "one-minus-src1", "src1-alpha", "one-minus-src1-alpha"];

const __wbindgen_enum_GpuBlendOperation = ["add", "subtract", "reverse-subtract", "min", "max"];

const __wbindgen_enum_GpuBufferBindingType = ["uniform", "storage", "read-only-storage"];

const __wbindgen_enum_GpuCanvasAlphaMode = ["opaque", "premultiplied"];

const __wbindgen_enum_GpuCompareFunction = ["never", "less", "equal", "less-equal", "greater", "not-equal", "greater-equal", "always"];

const __wbindgen_enum_GpuCullMode = ["none", "front", "back"];

const __wbindgen_enum_GpuErrorFilter = ["validation", "out-of-memory", "internal"];

const __wbindgen_enum_GpuFilterMode = ["nearest", "linear"];

const __wbindgen_enum_GpuFrontFace = ["ccw", "cw"];

const __wbindgen_enum_GpuIndexFormat = ["uint16", "uint32"];

const __wbindgen_enum_GpuLoadOp = ["load", "clear"];

const __wbindgen_enum_GpuMipmapFilterMode = ["nearest", "linear"];

const __wbindgen_enum_GpuPowerPreference = ["low-power", "high-performance"];

const __wbindgen_enum_GpuPrimitiveTopology = ["point-list", "line-list", "line-strip", "triangle-list", "triangle-strip"];

const __wbindgen_enum_GpuSamplerBindingType = ["filtering", "non-filtering", "comparison"];

const __wbindgen_enum_GpuStencilOperation = ["keep", "zero", "replace", "invert", "increment-clamp", "decrement-clamp", "increment-wrap", "decrement-wrap"];

const __wbindgen_enum_GpuStorageTextureAccess = ["write-only", "read-only", "read-write"];

const __wbindgen_enum_GpuStoreOp = ["store", "discard"];

const __wbindgen_enum_GpuTextureAspect = ["all", "stencil-only", "depth-only"];

const __wbindgen_enum_GpuTextureDimension = ["1d", "2d", "3d"];

const __wbindgen_enum_GpuTextureFormat = ["r8unorm", "r8snorm", "r8uint", "r8sint", "r16uint", "r16sint", "r16float", "rg8unorm", "rg8snorm", "rg8uint", "rg8sint", "r32uint", "r32sint", "r32float", "rg16uint", "rg16sint", "rg16float", "rgba8unorm", "rgba8unorm-srgb", "rgba8snorm", "rgba8uint", "rgba8sint", "bgra8unorm", "bgra8unorm-srgb", "rgb9e5ufloat", "rgb10a2uint", "rgb10a2unorm", "rg11b10ufloat", "rg32uint", "rg32sint", "rg32float", "rgba16uint", "rgba16sint", "rgba16float", "rgba32uint", "rgba32sint", "rgba32float", "stencil8", "depth16unorm", "depth24plus", "depth24plus-stencil8", "depth32float", "depth32float-stencil8", "bc1-rgba-unorm", "bc1-rgba-unorm-srgb", "bc2-rgba-unorm", "bc2-rgba-unorm-srgb", "bc3-rgba-unorm", "bc3-rgba-unorm-srgb", "bc4-r-unorm", "bc4-r-snorm", "bc5-rg-unorm", "bc5-rg-snorm", "bc6h-rgb-ufloat", "bc6h-rgb-float", "bc7-rgba-unorm", "bc7-rgba-unorm-srgb", "etc2-rgb8unorm", "etc2-rgb8unorm-srgb", "etc2-rgb8a1unorm", "etc2-rgb8a1unorm-srgb", "etc2-rgba8unorm", "etc2-rgba8unorm-srgb", "eac-r11unorm", "eac-r11snorm", "eac-rg11unorm", "eac-rg11snorm", "astc-4x4-unorm", "astc-4x4-unorm-srgb", "astc-5x4-unorm", "astc-5x4-unorm-srgb", "astc-5x5-unorm", "astc-5x5-unorm-srgb", "astc-6x5-unorm", "astc-6x5-unorm-srgb", "astc-6x6-unorm", "astc-6x6-unorm-srgb", "astc-8x5-unorm", "astc-8x5-unorm-srgb", "astc-8x6-unorm", "astc-8x6-unorm-srgb", "astc-8x8-unorm", "astc-8x8-unorm-srgb", "astc-10x5-unorm", "astc-10x5-unorm-srgb", "astc-10x6-unorm", "astc-10x6-unorm-srgb", "astc-10x8-unorm", "astc-10x8-unorm-srgb", "astc-10x10-unorm", "astc-10x10-unorm-srgb", "astc-12x10-unorm", "astc-12x10-unorm-srgb", "astc-12x12-unorm", "astc-12x12-unorm-srgb"];

const __wbindgen_enum_GpuTextureSampleType = ["float", "unfilterable-float", "depth", "sint", "uint"];

const __wbindgen_enum_GpuTextureViewDimension = ["1d", "2d", "2d-array", "cube", "cube-array", "3d"];

const __wbindgen_enum_GpuVertexFormat = ["uint8", "uint8x2", "uint8x4", "sint8", "sint8x2", "sint8x4", "unorm8", "unorm8x2", "unorm8x4", "snorm8", "snorm8x2", "snorm8x4", "uint16", "uint16x2", "uint16x4", "sint16", "sint16x2", "sint16x4", "unorm16", "unorm16x2", "unorm16x4", "snorm16", "snorm16x2", "snorm16x4", "float16", "float16x2", "float16x4", "float32", "float32x2", "float32x3", "float32x4", "uint32", "uint32x2", "uint32x3", "uint32x4", "sint32", "sint32x2", "sint32x3", "sint32x4", "unorm10-10-10-2", "unorm8x4-bgra"];

const __wbindgen_enum_GpuVertexStepMode = ["vertex", "instance"];

const __wbindgen_enum_ResizeObserverBoxOptions = ["border-box", "content-box", "device-pixel-content-box"];

const __wbindgen_enum_VisibilityState = ["hidden", "visible"];

const EXPECTED_RESPONSE_TYPES = new Set(['basic', 'cors', 'default']);

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);

            } catch (e) {
                const validResponse = module.ok && EXPECTED_RESPONSE_TYPES.has(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else {
                    throw e;
                }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);

    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };

        } else {
            return instance;
        }
    }
}

function __wbg_get_imports() {
    const imports = {};
    imports.wbg = {};
    imports.wbg.__wbg_Window_a4c5a48392f234ba = function(arg0) {
        const ret = arg0.Window;
        return ret;
    };
    imports.wbg.__wbg_Window_d1bf622f71ff0629 = function(arg0) {
        const ret = arg0.Window;
        return ret;
    };
    imports.wbg.__wbg_WorkerGlobalScope_2b2b89e1ac952b50 = function(arg0) {
        const ret = arg0.WorkerGlobalScope;
        return ret;
    };
    imports.wbg.__wbg_abort_67e1b49bf6614565 = function(arg0) {
        arg0.abort();
    };
    imports.wbg.__wbg_activeElement_da57789542a03158 = function(arg0) {
        const ret = arg0.activeElement;
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    imports.wbg.__wbg_addEventListener_775911544ac9d643 = function() { return handleError(function (arg0, arg1, arg2, arg3) {
        arg0.addEventListener(getStringFromWasm0(arg1, arg2), arg3);
    }, arguments) };
    imports.wbg.__wbg_addListener_0a8e8bf396edbcf2 = function() { return handleError(function (arg0, arg1) {
        arg0.addListener(arg1);
    }, arguments) };
    imports.wbg.__wbg_altKey_5ac2d88882a93598 = function(arg0) {
        const ret = arg0.altKey;
        return ret;
    };
    imports.wbg.__wbg_altKey_a8b663f4f5755ab0 = function(arg0) {
        const ret = arg0.altKey;
        return ret;
    };
    imports.wbg.__wbg_animate_6ec571f163cf6f8d = function(arg0, arg1, arg2) {
        const ret = arg0.animate(arg1, arg2);
        return ret;
    };
    imports.wbg.__wbg_appendChild_87a6cc0aeb132c06 = function() { return handleError(function (arg0, arg1) {
        const ret = arg0.appendChild(arg1);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_beginRenderPass_2bc62f5f78642ee0 = function() { return handleError(function (arg0, arg1) {
        const ret = arg0.beginRenderPass(arg1);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_blockSize_e6b651d3754c4602 = function(arg0) {
        const ret = arg0.blockSize;
        return ret;
    };
    imports.wbg.__wbg_body_8822ca55cb3730d2 = function(arg0) {
        const ret = arg0.body;
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    imports.wbg.__wbg_brand_9562792cbb4735c3 = function(arg0, arg1) {
        const ret = arg1.brand;
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg_brands_a1e7a2bce052128f = function(arg0) {
        const ret = arg0.brands;
        return ret;
    };
    imports.wbg.__wbg_buffer_8d40b1d762fb3c66 = function(arg0) {
        const ret = arg0.buffer;
        return ret;
    };
    imports.wbg.__wbg_button_47b736693b6dd97f = function(arg0) {
        const ret = arg0.button;
        return ret;
    };
    imports.wbg.__wbg_buttons_acf5180ad8f6ae06 = function(arg0) {
        const ret = arg0.buttons;
        return ret;
    };
    imports.wbg.__wbg_call_13410aac570ffff7 = function() { return handleError(function (arg0, arg1) {
        const ret = arg0.call(arg1);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_call_641db1bb5db5a579 = function() { return handleError(function (arg0, arg1, arg2, arg3) {
        const ret = arg0.call(arg1, arg2, arg3);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_cancelAnimationFrame_a3b3c0d5b5c0056d = function() { return handleError(function (arg0, arg1) {
        arg0.cancelAnimationFrame(arg1);
    }, arguments) };
    imports.wbg.__wbg_cancelIdleCallback_fea12ddf6a573e29 = function(arg0, arg1) {
        arg0.cancelIdleCallback(arg1 >>> 0);
    };
    imports.wbg.__wbg_cancel_09c394f0894744eb = function(arg0) {
        arg0.cancel();
    };
    imports.wbg.__wbg_catch_c80ecae90cb8ed4e = function(arg0, arg1) {
        const ret = arg0.catch(arg1);
        return ret;
    };
    imports.wbg.__wbg_clearTimeout_50a601223dd18306 = function(arg0, arg1) {
        arg0.clearTimeout(arg1);
    };
    imports.wbg.__wbg_close_19bc0b26bb42f409 = function(arg0) {
        arg0.close();
    };
    imports.wbg.__wbg_code_9c657b2df9e85331 = function(arg0, arg1) {
        const ret = arg1.code;
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg_configure_bced8e40e8dbaaa0 = function() { return handleError(function (arg0, arg1) {
        arg0.configure(arg1);
    }, arguments) };
    imports.wbg.__wbg_contains_d71a802f20288218 = function(arg0, arg1) {
        const ret = arg0.contains(arg1);
        return ret;
    };
    imports.wbg.__wbg_contentRect_4fac166d7cf7a578 = function(arg0) {
        const ret = arg0.contentRect;
        return ret;
    };
    imports.wbg.__wbg_createBindGroupLayout_3fb59c14aed4b64e = function() { return handleError(function (arg0, arg1) {
        const ret = arg0.createBindGroupLayout(arg1);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_createBindGroup_03f26b8770895116 = function(arg0, arg1) {
        const ret = arg0.createBindGroup(arg1);
        return ret;
    };
    imports.wbg.__wbg_createBuffer_76f7598789ecc3d7 = function() { return handleError(function (arg0, arg1) {
        const ret = arg0.createBuffer(arg1);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_createCommandEncoder_f8056019328bd192 = function(arg0, arg1) {
        const ret = arg0.createCommandEncoder(arg1);
        return ret;
    };
    imports.wbg.__wbg_createElement_4909dfa2011f2abe = function() { return handleError(function (arg0, arg1, arg2) {
        const ret = arg0.createElement(getStringFromWasm0(arg1, arg2));
        return ret;
    }, arguments) };
    imports.wbg.__wbg_createObjectURL_c80225986d2b928b = function() { return handleError(function (arg0, arg1) {
        const ret = URL.createObjectURL(arg1);
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    }, arguments) };
    imports.wbg.__wbg_createPipelineLayout_5039b0679b6b7f36 = function(arg0, arg1) {
        const ret = arg0.createPipelineLayout(arg1);
        return ret;
    };
    imports.wbg.__wbg_createRenderPipeline_db585efa9bab66f3 = function() { return handleError(function (arg0, arg1) {
        const ret = arg0.createRenderPipeline(arg1);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_createSampler_e421d07197c6e5ec = function(arg0, arg1) {
        const ret = arg0.createSampler(arg1);
        return ret;
    };
    imports.wbg.__wbg_createShaderModule_3facfe98356b79a9 = function(arg0, arg1) {
        const ret = arg0.createShaderModule(arg1);
        return ret;
    };
    imports.wbg.__wbg_createTexture_49002c91188f6137 = function() { return handleError(function (arg0, arg1) {
        const ret = arg0.createTexture(arg1);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_createView_0ce5c82d78f482df = function() { return handleError(function (arg0, arg1) {
        const ret = arg0.createView(arg1);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_ctrlKey_d6452dce5a5af017 = function(arg0) {
        const ret = arg0.ctrlKey;
        return ret;
    };
    imports.wbg.__wbg_ctrlKey_d85b3ef2e41e6483 = function(arg0) {
        const ret = arg0.ctrlKey;
        return ret;
    };
    imports.wbg.__wbg_debug_c906769d2f88c17b = function(arg0) {
        console.debug(arg0);
    };
    imports.wbg.__wbg_deltaMode_b39c7bf656cadad6 = function(arg0) {
        const ret = arg0.deltaMode;
        return ret;
    };
    imports.wbg.__wbg_deltaX_e639e6be7245bedc = function(arg0) {
        const ret = arg0.deltaX;
        return ret;
    };
    imports.wbg.__wbg_deltaY_2d352968f40fb71a = function(arg0) {
        const ret = arg0.deltaY;
        return ret;
    };
    imports.wbg.__wbg_devicePixelContentBoxSize_6e79a8ed6b36cd2c = function(arg0) {
        const ret = arg0.devicePixelContentBoxSize;
        return ret;
    };
    imports.wbg.__wbg_devicePixelRatio_de772f7b570607fa = function(arg0) {
        const ret = arg0.devicePixelRatio;
        return ret;
    };
    imports.wbg.__wbg_disconnect_240ad3fbb76010b8 = function(arg0) {
        arg0.disconnect();
    };
    imports.wbg.__wbg_disconnect_9a156f531f823310 = function(arg0) {
        arg0.disconnect();
    };
    imports.wbg.__wbg_document_7d29d139bd619045 = function(arg0) {
        const ret = arg0.document;
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    imports.wbg.__wbg_drawIndexed_d1202dc1fe88d5f5 = function(arg0, arg1, arg2, arg3, arg4, arg5) {
        arg0.drawIndexed(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4, arg5 >>> 0);
    };
    imports.wbg.__wbg_draw_d3b53fbcc9853635 = function(arg0, arg1, arg2, arg3, arg4) {
        arg0.draw(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4 >>> 0);
    };
    imports.wbg.__wbg_end_b9d7079f54620f76 = function(arg0) {
        arg0.end();
    };
    imports.wbg.__wbg_error_4700bbeb78363714 = function(arg0, arg1) {
        console.error(arg0, arg1);
    };
    imports.wbg.__wbg_error_7534b8e9a36f1ab4 = function(arg0, arg1) {
        let deferred0_0;
        let deferred0_1;
        try {
            deferred0_0 = arg0;
            deferred0_1 = arg1;
            console.error(getStringFromWasm0(arg0, arg1));
        } finally {
            wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
        }
    };
    imports.wbg.__wbg_error_99981e16d476aa5c = function(arg0) {
        console.error(arg0);
    };
    imports.wbg.__wbg_error_d2e248d3e80a06c7 = function(arg0) {
        const ret = arg0.error;
        return ret;
    };
    imports.wbg.__wbg_eval_631dc78d65611b94 = function() { return handleError(function (arg0, arg1) {
        const ret = eval(getStringFromWasm0(arg0, arg1));
        return ret;
    }, arguments) };
    imports.wbg.__wbg_features_1e615dfe5ee66265 = function(arg0) {
        const ret = arg0.features;
        return ret;
    };
    imports.wbg.__wbg_features_23875e4e632c481d = function(arg0) {
        const ret = arg0.features;
        return ret;
    };
    imports.wbg.__wbg_finish_17a0b297901010d5 = function(arg0) {
        const ret = arg0.finish();
        return ret;
    };
    imports.wbg.__wbg_finish_ab9e01a922269f3a = function(arg0, arg1) {
        const ret = arg0.finish(arg1);
        return ret;
    };
    imports.wbg.__wbg_focus_8541343802c6721b = function() { return handleError(function (arg0) {
        arg0.focus();
    }, arguments) };
    imports.wbg.__wbg_fullscreenElement_46b4e1ed8248950d = function(arg0) {
        const ret = arg0.fullscreenElement;
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    imports.wbg.__wbg_getCoalescedEvents_14265d8b5a22b7c5 = function(arg0) {
        const ret = arg0.getCoalescedEvents();
        return ret;
    };
    imports.wbg.__wbg_getCoalescedEvents_21492912fd0145ec = function(arg0) {
        const ret = arg0.getCoalescedEvents;
        return ret;
    };
    imports.wbg.__wbg_getComputedStyle_06167bcde483501e = function() { return handleError(function (arg0, arg1) {
        const ret = arg0.getComputedStyle(arg1);
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    }, arguments) };
    imports.wbg.__wbg_getContext_15e158d04230a6f6 = function() { return handleError(function (arg0, arg1, arg2) {
        const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    }, arguments) };
    imports.wbg.__wbg_getContext_8b08935510bf607c = function() { return handleError(function (arg0, arg1, arg2) {
        const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    }, arguments) };
    imports.wbg.__wbg_getCurrentTexture_d64323b76f42d5e0 = function() { return handleError(function (arg0) {
        const ret = arg0.getCurrentTexture();
        return ret;
    }, arguments) };
    imports.wbg.__wbg_getMappedRange_1229810ff58e27ce = function() { return handleError(function (arg0, arg1, arg2) {
        const ret = arg0.getMappedRange(arg1, arg2);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_getOwnPropertyDescriptor_b58e9c0d5f644b26 = function(arg0, arg1) {
        const ret = Object.getOwnPropertyDescriptor(arg0, arg1);
        return ret;
    };
    imports.wbg.__wbg_getPreferredCanvasFormat_9aef34efead2aa08 = function(arg0) {
        const ret = arg0.getPreferredCanvasFormat();
        return (__wbindgen_enum_GpuTextureFormat.indexOf(ret) + 1 || 96) - 1;
    };
    imports.wbg.__wbg_getPropertyValue_da119dca19ff1bd7 = function() { return handleError(function (arg0, arg1, arg2, arg3) {
        const ret = arg1.getPropertyValue(getStringFromWasm0(arg2, arg3));
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    }, arguments) };
    imports.wbg.__wbg_getRandomValues_3c9c0d586e575a16 = function() { return handleError(function (arg0, arg1) {
        globalThis.crypto.getRandomValues(getArrayU8FromWasm0(arg0, arg1));
    }, arguments) };
    imports.wbg.__wbg_get_0da715ceaecea5c8 = function(arg0, arg1) {
        const ret = arg0[arg1 >>> 0];
        return ret;
    };
    imports.wbg.__wbg_get_2a8fe8079145daf7 = function(arg0, arg1) {
        const ret = arg0[arg1 >>> 0];
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    imports.wbg.__wbg_get_458e874b43b18b25 = function() { return handleError(function (arg0, arg1) {
        const ret = Reflect.get(arg0, arg1);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_gpu_a6bce2913fb8f574 = function(arg0) {
        const ret = arg0.gpu;
        return ret;
    };
    imports.wbg.__wbg_has_4891bec062ded753 = function(arg0, arg1, arg2) {
        const ret = arg0.has(getStringFromWasm0(arg1, arg2));
        return ret;
    };
    imports.wbg.__wbg_height_4b1c53fac682bfa2 = function(arg0) {
        const ret = arg0.height;
        return ret;
    };
    imports.wbg.__wbg_info_6cf68c1a86a92f6a = function(arg0) {
        console.info(arg0);
    };
    imports.wbg.__wbg_inlineSize_28bb3208ec333a04 = function(arg0) {
        const ret = arg0.inlineSize;
        return ret;
    };
    imports.wbg.__wbg_instanceof_GpuAdapter_fb230cdccb184887 = function(arg0) {
        let result;
        try {
            result = arg0 instanceof GPUAdapter;
        } catch (_) {
            result = false;
        }
        const ret = result;
        return ret;
    };
    imports.wbg.__wbg_instanceof_GpuCanvasContext_48ec5330c4425d84 = function(arg0) {
        let result;
        try {
            result = arg0 instanceof GPUCanvasContext;
        } catch (_) {
            result = false;
        }
        const ret = result;
        return ret;
    };
    imports.wbg.__wbg_instanceof_GpuOutOfMemoryError_7b1554dded417c4f = function(arg0) {
        let result;
        try {
            result = arg0 instanceof GPUOutOfMemoryError;
        } catch (_) {
            result = false;
        }
        const ret = result;
        return ret;
    };
    imports.wbg.__wbg_instanceof_GpuValidationError_124458c5a6bb3b58 = function(arg0) {
        let result;
        try {
            result = arg0 instanceof GPUValidationError;
        } catch (_) {
            result = false;
        }
        const ret = result;
        return ret;
    };
    imports.wbg.__wbg_instanceof_Object_fbf5fef4952ff29b = function(arg0) {
        let result;
        try {
            result = arg0 instanceof Object;
        } catch (_) {
            result = false;
        }
        const ret = result;
        return ret;
    };
    imports.wbg.__wbg_instanceof_Promise_3ec9e849bf41bdb6 = function(arg0) {
        let result;
        try {
            result = arg0 instanceof Promise;
        } catch (_) {
            result = false;
        }
        const ret = result;
        return ret;
    };
    imports.wbg.__wbg_instanceof_Uint8Array_9a8378d955933db7 = function(arg0) {
        let result;
        try {
            result = arg0 instanceof Uint8Array;
        } catch (_) {
            result = false;
        }
        const ret = result;
        return ret;
    };
    imports.wbg.__wbg_instanceof_Window_12d20d558ef92592 = function(arg0) {
        let result;
        try {
            result = arg0 instanceof Window;
        } catch (_) {
            result = false;
        }
        const ret = result;
        return ret;
    };
    imports.wbg.__wbg_isIntersecting_31dfa252ee048a6f = function(arg0) {
        const ret = arg0.isIntersecting;
        return ret;
    };
    imports.wbg.__wbg_is_8346b6c36feaf71a = function(arg0, arg1) {
        const ret = Object.is(arg0, arg1);
        return ret;
    };
    imports.wbg.__wbg_key_caac8fafdd6d5317 = function(arg0, arg1) {
        const ret = arg1.key;
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg_label_cda985b32d44cee0 = function(arg0, arg1) {
        const ret = arg1.label;
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg_length_186546c51cd61acd = function(arg0) {
        const ret = arg0.length;
        return ret;
    };
    imports.wbg.__wbg_length_6bb7e81f9d7713e4 = function(arg0) {
        const ret = arg0.length;
        return ret;
    };
    imports.wbg.__wbg_limits_b6a82fde3977821c = function(arg0) {
        const ret = arg0.limits;
        return ret;
    };
    imports.wbg.__wbg_location_b9a1887cb231e862 = function(arg0) {
        const ret = arg0.location;
        return ret;
    };
    imports.wbg.__wbg_log_6c7b5f4f00b8ce3f = function(arg0) {
        console.log(arg0);
    };
    imports.wbg.__wbg_matchMedia_19600e31a5612b23 = function() { return handleError(function (arg0, arg1, arg2) {
        const ret = arg0.matchMedia(getStringFromWasm0(arg1, arg2));
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    }, arguments) };
    imports.wbg.__wbg_matches_12ebc1caa30f1e42 = function(arg0) {
        const ret = arg0.matches;
        return ret;
    };
    imports.wbg.__wbg_maxBindGroups_c88520bb1d32bb51 = function(arg0) {
        const ret = arg0.maxBindGroups;
        return ret;
    };
    imports.wbg.__wbg_maxBindingsPerBindGroup_432afc05fd4d1473 = function(arg0) {
        const ret = arg0.maxBindingsPerBindGroup;
        return ret;
    };
    imports.wbg.__wbg_maxBufferSize_b67a4c44cc76ddc3 = function(arg0) {
        const ret = arg0.maxBufferSize;
        return ret;
    };
    imports.wbg.__wbg_maxColorAttachmentBytesPerSample_2b29886758adffb4 = function(arg0) {
        const ret = arg0.maxColorAttachmentBytesPerSample;
        return ret;
    };
    imports.wbg.__wbg_maxColorAttachments_ec0f3f73d0af16a4 = function(arg0) {
        const ret = arg0.maxColorAttachments;
        return ret;
    };
    imports.wbg.__wbg_maxComputeInvocationsPerWorkgroup_ea57344834f1a195 = function(arg0) {
        const ret = arg0.maxComputeInvocationsPerWorkgroup;
        return ret;
    };
    imports.wbg.__wbg_maxComputeWorkgroupSizeX_b924545971550146 = function(arg0) {
        const ret = arg0.maxComputeWorkgroupSizeX;
        return ret;
    };
    imports.wbg.__wbg_maxComputeWorkgroupSizeY_c0d9d68b1acecdc1 = function(arg0) {
        const ret = arg0.maxComputeWorkgroupSizeY;
        return ret;
    };
    imports.wbg.__wbg_maxComputeWorkgroupSizeZ_3898cfa28ca6d14f = function(arg0) {
        const ret = arg0.maxComputeWorkgroupSizeZ;
        return ret;
    };
    imports.wbg.__wbg_maxComputeWorkgroupStorageSize_edea548daf4af87d = function(arg0) {
        const ret = arg0.maxComputeWorkgroupStorageSize;
        return ret;
    };
    imports.wbg.__wbg_maxComputeWorkgroupsPerDimension_bfc346c1292145d9 = function(arg0) {
        const ret = arg0.maxComputeWorkgroupsPerDimension;
        return ret;
    };
    imports.wbg.__wbg_maxDynamicStorageBuffersPerPipelineLayout_e7359e7bdfc76801 = function(arg0) {
        const ret = arg0.maxDynamicStorageBuffersPerPipelineLayout;
        return ret;
    };
    imports.wbg.__wbg_maxDynamicUniformBuffersPerPipelineLayout_8beefcf6b6ae3a02 = function(arg0) {
        const ret = arg0.maxDynamicUniformBuffersPerPipelineLayout;
        return ret;
    };
    imports.wbg.__wbg_maxSampledTexturesPerShaderStage_7fe798e58a892ea4 = function(arg0) {
        const ret = arg0.maxSampledTexturesPerShaderStage;
        return ret;
    };
    imports.wbg.__wbg_maxSamplersPerShaderStage_84408cd7914be213 = function(arg0) {
        const ret = arg0.maxSamplersPerShaderStage;
        return ret;
    };
    imports.wbg.__wbg_maxStorageBufferBindingSize_9711b12549c371a6 = function(arg0) {
        const ret = arg0.maxStorageBufferBindingSize;
        return ret;
    };
    imports.wbg.__wbg_maxStorageBuffersPerShaderStage_3b626e8ff1584e0b = function(arg0) {
        const ret = arg0.maxStorageBuffersPerShaderStage;
        return ret;
    };
    imports.wbg.__wbg_maxStorageTexturesPerShaderStage_c612c8e8f36e7ad3 = function(arg0) {
        const ret = arg0.maxStorageTexturesPerShaderStage;
        return ret;
    };
    imports.wbg.__wbg_maxTextureArrayLayers_6e0973f615982bee = function(arg0) {
        const ret = arg0.maxTextureArrayLayers;
        return ret;
    };
    imports.wbg.__wbg_maxTextureDimension1D_fda090a895ffead5 = function(arg0) {
        const ret = arg0.maxTextureDimension1D;
        return ret;
    };
    imports.wbg.__wbg_maxTextureDimension2D_876dc9c39fa8de4e = function(arg0) {
        const ret = arg0.maxTextureDimension2D;
        return ret;
    };
    imports.wbg.__wbg_maxTextureDimension3D_3e8ca51b995bc0e0 = function(arg0) {
        const ret = arg0.maxTextureDimension3D;
        return ret;
    };
    imports.wbg.__wbg_maxUniformBufferBindingSize_d9898f62e702922b = function(arg0) {
        const ret = arg0.maxUniformBufferBindingSize;
        return ret;
    };
    imports.wbg.__wbg_maxUniformBuffersPerShaderStage_80346a93791c45ff = function(arg0) {
        const ret = arg0.maxUniformBuffersPerShaderStage;
        return ret;
    };
    imports.wbg.__wbg_maxVertexAttributes_bb30494dbeda4a16 = function(arg0) {
        const ret = arg0.maxVertexAttributes;
        return ret;
    };
    imports.wbg.__wbg_maxVertexBufferArrayStride_f2b103ca29d68d1a = function(arg0) {
        const ret = arg0.maxVertexBufferArrayStride;
        return ret;
    };
    imports.wbg.__wbg_maxVertexBuffers_522f56407d841954 = function(arg0) {
        const ret = arg0.maxVertexBuffers;
        return ret;
    };
    imports.wbg.__wbg_media_ec233ae1b636ce31 = function(arg0, arg1) {
        const ret = arg1.media;
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg_message_37510c9867df2726 = function(arg0, arg1) {
        const ret = arg1.message;
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg_metaKey_3faf4a14e870c3d6 = function(arg0) {
        const ret = arg0.metaKey;
        return ret;
    };
    imports.wbg.__wbg_metaKey_48d6907eef50622b = function(arg0) {
        const ret = arg0.metaKey;
        return ret;
    };
    imports.wbg.__wbg_minStorageBufferOffsetAlignment_8150d07a1d4bf231 = function(arg0) {
        const ret = arg0.minStorageBufferOffsetAlignment;
        return ret;
    };
    imports.wbg.__wbg_minUniformBufferOffsetAlignment_f2960fb3c8ad86bd = function(arg0) {
        const ret = arg0.minUniformBufferOffsetAlignment;
        return ret;
    };
    imports.wbg.__wbg_movementX_0ef0e79f7b9168fc = function(arg0) {
        const ret = arg0.movementX;
        return ret;
    };
    imports.wbg.__wbg_movementY_875c2fc2aabd99bf = function(arg0) {
        const ret = arg0.movementY;
        return ret;
    };
    imports.wbg.__wbg_navigator_65d5ad763926b868 = function(arg0) {
        const ret = arg0.navigator;
        return ret;
    };
    imports.wbg.__wbg_navigator_bfaf1b0b0eb48da2 = function(arg0) {
        const ret = arg0.navigator;
        return ret;
    };
    imports.wbg.__wbg_new_19c25a3f2fa63a02 = function() {
        const ret = new Object();
        return ret;
    };
    imports.wbg.__wbg_new_1f3a344cf3123716 = function() {
        const ret = new Array();
        return ret;
    };
    imports.wbg.__wbg_new_3955c3ac4df3b2a7 = function() { return handleError(function () {
        const ret = new MessageChannel();
        return ret;
    }, arguments) };
    imports.wbg.__wbg_new_66b9434b4e59b63e = function() { return handleError(function () {
        const ret = new AbortController();
        return ret;
    }, arguments) };
    imports.wbg.__wbg_new_8a6f238a6ece86ea = function() {
        const ret = new Error();
        return ret;
    };
    imports.wbg.__wbg_new_9d476835fd376de6 = function() { return handleError(function (arg0, arg1) {
        const ret = new Worker(getStringFromWasm0(arg0, arg1));
        return ret;
    }, arguments) };
    imports.wbg.__wbg_new_a306f04fbb289085 = function() { return handleError(function (arg0) {
        const ret = new IntersectionObserver(arg0);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_new_c949fe92f1151b4b = function() { return handleError(function (arg0) {
        const ret = new ResizeObserver(arg0);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_newfromslice_074c56947bd43469 = function(arg0, arg1) {
        const ret = new Uint8Array(getArrayU8FromWasm0(arg0, arg1));
        return ret;
    };
    imports.wbg.__wbg_newnoargs_254190557c45b4ec = function(arg0, arg1) {
        const ret = new Function(getStringFromWasm0(arg0, arg1));
        return ret;
    };
    imports.wbg.__wbg_newwithbyteoffsetandlength_e8f53910b4d42b45 = function(arg0, arg1, arg2) {
        const ret = new Uint8Array(arg0, arg1 >>> 0, arg2 >>> 0);
        return ret;
    };
    imports.wbg.__wbg_newwithstrsequenceandoptions_5b257525e688af7d = function() { return handleError(function (arg0, arg1) {
        const ret = new Blob(arg0, arg1);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_now_2c95c9de01293173 = function(arg0) {
        const ret = arg0.now();
        return ret;
    };
    imports.wbg.__wbg_observe_1191c7319883ed4f = function(arg0, arg1, arg2) {
        arg0.observe(arg1, arg2);
    };
    imports.wbg.__wbg_observe_7c27e1799599c503 = function(arg0, arg1) {
        arg0.observe(arg1);
    };
    imports.wbg.__wbg_observe_d5620e0d99e20a09 = function(arg0, arg1) {
        arg0.observe(arg1);
    };
    imports.wbg.__wbg_of_30e97a7ad6e3518b = function(arg0) {
        const ret = Array.of(arg0);
        return ret;
    };
    imports.wbg.__wbg_of_d0e190785e1ebbb6 = function(arg0, arg1) {
        const ret = Array.of(arg0, arg1);
        return ret;
    };
    imports.wbg.__wbg_offsetX_cb6a38e6f23cb4a6 = function(arg0) {
        const ret = arg0.offsetX;
        return ret;
    };
    imports.wbg.__wbg_offsetY_43e21941c5c1f8bf = function(arg0) {
        const ret = arg0.offsetY;
        return ret;
    };
    imports.wbg.__wbg_parentElement_48440a9a6ba034a8 = function(arg0) {
        const ret = arg0.parentElement;
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    imports.wbg.__wbg_performance_7a3ffd0b17f663ad = function(arg0) {
        const ret = arg0.performance;
        return ret;
    };
    imports.wbg.__wbg_persisted_13ec492f01565fa5 = function(arg0) {
        const ret = arg0.persisted;
        return ret;
    };
    imports.wbg.__wbg_play_63bc12f42e16af91 = function(arg0) {
        arg0.play();
    };
    imports.wbg.__wbg_pointerId_6bf7f6b01d55295b = function(arg0) {
        const ret = arg0.pointerId;
        return ret;
    };
    imports.wbg.__wbg_pointerType_7853885b50da7698 = function(arg0, arg1) {
        const ret = arg1.pointerType;
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg_popErrorScope_37df7a5518e8b238 = function(arg0) {
        const ret = arg0.popErrorScope();
        return ret;
    };
    imports.wbg.__wbg_port1_9bfdb70bf361065d = function(arg0) {
        const ret = arg0.port1;
        return ret;
    };
    imports.wbg.__wbg_port2_4dad3e9374ecfa63 = function(arg0) {
        const ret = arg0.port2;
        return ret;
    };
    imports.wbg.__wbg_postMessage_0e07159f7ab2518e = function() { return handleError(function (arg0, arg1) {
        arg0.postMessage(arg1);
    }, arguments) };
    imports.wbg.__wbg_postMessage_775fdcd73133235f = function() { return handleError(function (arg0, arg1, arg2) {
        arg0.postMessage(arg1, arg2);
    }, arguments) };
    imports.wbg.__wbg_postTask_41d93e93941e4a3d = function(arg0, arg1, arg2) {
        const ret = arg0.postTask(arg1, arg2);
        return ret;
    };
    imports.wbg.__wbg_pressure_eed52402b4cd13a9 = function(arg0) {
        const ret = arg0.pressure;
        return ret;
    };
    imports.wbg.__wbg_preventDefault_fab9a085b3006058 = function(arg0) {
        arg0.preventDefault();
    };
    imports.wbg.__wbg_prototype_c28bca39c45aba9b = function() {
        const ret = ResizeObserverEntry.prototype;
        return ret;
    };
    imports.wbg.__wbg_prototypesetcall_3d4a26c1ed734349 = function(arg0, arg1, arg2) {
        Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
    };
    imports.wbg.__wbg_pushErrorScope_79bafe9790e688cf = function(arg0, arg1) {
        arg0.pushErrorScope(__wbindgen_enum_GpuErrorFilter[arg1]);
    };
    imports.wbg.__wbg_push_330b2eb93e4e1212 = function(arg0, arg1) {
        const ret = arg0.push(arg1);
        return ret;
    };
    imports.wbg.__wbg_querySelectorAll_71b924f0e83d096b = function() { return handleError(function (arg0, arg1, arg2) {
        const ret = arg0.querySelectorAll(getStringFromWasm0(arg1, arg2));
        return ret;
    }, arguments) };
    imports.wbg.__wbg_queueMicrotask_19632579d2a4b200 = function(arg0, arg1) {
        arg0.queueMicrotask(arg1);
    };
    imports.wbg.__wbg_queueMicrotask_25d0739ac89e8c88 = function(arg0) {
        queueMicrotask(arg0);
    };
    imports.wbg.__wbg_queueMicrotask_4488407636f5bf24 = function(arg0) {
        const ret = arg0.queueMicrotask;
        return ret;
    };
    imports.wbg.__wbg_queue_39d4f3bda761adef = function(arg0) {
        const ret = arg0.queue;
        return ret;
    };
    imports.wbg.__wbg_removeEventListener_6d5be9c2821a511e = function() { return handleError(function (arg0, arg1, arg2, arg3) {
        arg0.removeEventListener(getStringFromWasm0(arg1, arg2), arg3);
    }, arguments) };
    imports.wbg.__wbg_removeListener_182fbcdf2441ee87 = function() { return handleError(function (arg0, arg1) {
        arg0.removeListener(arg1);
    }, arguments) };
    imports.wbg.__wbg_removeProperty_8912427f4d0f6361 = function() { return handleError(function (arg0, arg1, arg2, arg3) {
        const ret = arg1.removeProperty(getStringFromWasm0(arg2, arg3));
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    }, arguments) };
    imports.wbg.__wbg_repeat_fa67d2825d6e556f = function(arg0) {
        const ret = arg0.repeat;
        return ret;
    };
    imports.wbg.__wbg_requestAdapter_55d15e6d14e8392c = function(arg0, arg1) {
        const ret = arg0.requestAdapter(arg1);
        return ret;
    };
    imports.wbg.__wbg_requestAnimationFrame_ddc84a7def436784 = function() { return handleError(function (arg0, arg1) {
        const ret = arg0.requestAnimationFrame(arg1);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_requestDevice_66e864eaf1ffbb38 = function(arg0, arg1) {
        const ret = arg0.requestDevice(arg1);
        return ret;
    };
    imports.wbg.__wbg_requestFullscreen_86fc6cdb76000482 = function(arg0) {
        const ret = arg0.requestFullscreen;
        return ret;
    };
    imports.wbg.__wbg_requestFullscreen_9f0611438eb929cf = function(arg0) {
        const ret = arg0.requestFullscreen();
        return ret;
    };
    imports.wbg.__wbg_requestIdleCallback_1b8d644ff564208f = function(arg0) {
        const ret = arg0.requestIdleCallback;
        return ret;
    };
    imports.wbg.__wbg_requestIdleCallback_de48528fbbe27518 = function() { return handleError(function (arg0, arg1) {
        const ret = arg0.requestIdleCallback(arg1);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_resolve_4055c623acdd6a1b = function(arg0) {
        const ret = Promise.resolve(arg0);
        return ret;
    };
    imports.wbg.__wbg_revokeObjectURL_651b859c81bf1af0 = function() { return handleError(function (arg0, arg1) {
        URL.revokeObjectURL(getStringFromWasm0(arg0, arg1));
    }, arguments) };
    imports.wbg.__wbg_scheduler_48482a9974eeacbd = function(arg0) {
        const ret = arg0.scheduler;
        return ret;
    };
    imports.wbg.__wbg_scheduler_5156bb61cc1cf589 = function(arg0) {
        const ret = arg0.scheduler;
        return ret;
    };
    imports.wbg.__wbg_setAttribute_d1baf9023ad5696f = function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
        arg0.setAttribute(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
    }, arguments) };
    imports.wbg.__wbg_setBindGroup_77fc1c2c49ddcff0 = function(arg0, arg1, arg2) {
        arg0.setBindGroup(arg1 >>> 0, arg2);
    };
    imports.wbg.__wbg_setBindGroup_b966448206045bdd = function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
        arg0.setBindGroup(arg1 >>> 0, arg2, getArrayU32FromWasm0(arg3, arg4), arg5, arg6 >>> 0);
    }, arguments) };
    imports.wbg.__wbg_setIndexBuffer_8282bd9ab99d7946 = function(arg0, arg1, arg2, arg3, arg4) {
        arg0.setIndexBuffer(arg1, __wbindgen_enum_GpuIndexFormat[arg2], arg3, arg4);
    };
    imports.wbg.__wbg_setPipeline_6dd7dffa6e7d7496 = function(arg0, arg1) {
        arg0.setPipeline(arg1);
    };
    imports.wbg.__wbg_setPointerCapture_50d37dfb10244aba = function() { return handleError(function (arg0, arg1) {
        arg0.setPointerCapture(arg1);
    }, arguments) };
    imports.wbg.__wbg_setProperty_a4431938dd3e6945 = function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
        arg0.setProperty(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
    }, arguments) };
    imports.wbg.__wbg_setTimeout_2966518f28aef92e = function() { return handleError(function (arg0, arg1, arg2) {
        const ret = arg0.setTimeout(arg1, arg2);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_setTimeout_b3de16ce9694711c = function() { return handleError(function (arg0, arg1) {
        const ret = arg0.setTimeout(arg1);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_setVertexBuffer_b562a8a167090c01 = function(arg0, arg1, arg2, arg3, arg4) {
        arg0.setVertexBuffer(arg1 >>> 0, arg2, arg3, arg4);
    };
    imports.wbg.__wbg_set_453345bcda80b89a = function() { return handleError(function (arg0, arg1, arg2) {
        const ret = Reflect.set(arg0, arg1, arg2);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_set_a8d46c21281c7551 = function(arg0, arg1, arg2) {
        arg0.set(arg1, arg2 >>> 0);
    };
    imports.wbg.__wbg_seta_add312ccdfbfaa2d = function(arg0, arg1) {
        arg0.a = arg1;
    };
    imports.wbg.__wbg_setaccess_c87a9bdb5c449e6b = function(arg0, arg1) {
        arg0.access = __wbindgen_enum_GpuStorageTextureAccess[arg1];
    };
    imports.wbg.__wbg_setaddressmodeu_2ff1a762cca3e679 = function(arg0, arg1) {
        arg0.addressModeU = __wbindgen_enum_GpuAddressMode[arg1];
    };
    imports.wbg.__wbg_setaddressmodev_1322b1b0dafa29ef = function(arg0, arg1) {
        arg0.addressModeV = __wbindgen_enum_GpuAddressMode[arg1];
    };
    imports.wbg.__wbg_setaddressmodew_1128071f5dcb4e54 = function(arg0, arg1) {
        arg0.addressModeW = __wbindgen_enum_GpuAddressMode[arg1];
    };
    imports.wbg.__wbg_setalpha_23751af59d391d98 = function(arg0, arg1) {
        arg0.alpha = arg1;
    };
    imports.wbg.__wbg_setalphamode_1192a40e9bd8c3aa = function(arg0, arg1) {
        arg0.alphaMode = __wbindgen_enum_GpuCanvasAlphaMode[arg1];
    };
    imports.wbg.__wbg_setalphatocoverageenabled_9700e84c77d52727 = function(arg0, arg1) {
        arg0.alphaToCoverageEnabled = arg1 !== 0;
    };
    imports.wbg.__wbg_setarraylayercount_3a8ad1adab3aded1 = function(arg0, arg1) {
        arg0.arrayLayerCount = arg1 >>> 0;
    };
    imports.wbg.__wbg_setarraystride_5508d074b809d568 = function(arg0, arg1) {
        arg0.arrayStride = arg1;
    };
    imports.wbg.__wbg_setaspect_4066a62e6528c589 = function(arg0, arg1) {
        arg0.aspect = __wbindgen_enum_GpuTextureAspect[arg1];
    };
    imports.wbg.__wbg_setattributes_aa15086089274167 = function(arg0, arg1) {
        arg0.attributes = arg1;
    };
    imports.wbg.__wbg_setb_162f487856c3bad9 = function(arg0, arg1) {
        arg0.b = arg1;
    };
    imports.wbg.__wbg_setbasearraylayer_85c4780859e3e025 = function(arg0, arg1) {
        arg0.baseArrayLayer = arg1 >>> 0;
    };
    imports.wbg.__wbg_setbasemiplevel_f90525112a282a1d = function(arg0, arg1) {
        arg0.baseMipLevel = arg1 >>> 0;
    };
    imports.wbg.__wbg_setbeginningofpasswriteindex_c8a62bc66645f5cd = function(arg0, arg1) {
        arg0.beginningOfPassWriteIndex = arg1 >>> 0;
    };
    imports.wbg.__wbg_setbindgrouplayouts_54f980eb55071c87 = function(arg0, arg1) {
        arg0.bindGroupLayouts = arg1;
    };
    imports.wbg.__wbg_setbinding_1ddbf5eebabdc48c = function(arg0, arg1) {
        arg0.binding = arg1 >>> 0;
    };
    imports.wbg.__wbg_setbinding_5ea4d52c77434dfa = function(arg0, arg1) {
        arg0.binding = arg1 >>> 0;
    };
    imports.wbg.__wbg_setblend_4a45a53ea0e4706e = function(arg0, arg1) {
        arg0.blend = arg1;
    };
    imports.wbg.__wbg_setbox_3751928f4f6acf2f = function(arg0, arg1) {
        arg0.box = __wbindgen_enum_ResizeObserverBoxOptions[arg1];
    };
    imports.wbg.__wbg_setbuffer_2dac3e64a7099038 = function(arg0, arg1) {
        arg0.buffer = arg1;
    };
    imports.wbg.__wbg_setbuffer_489d923366e1f63a = function(arg0, arg1) {
        arg0.buffer = arg1;
    };
    imports.wbg.__wbg_setbuffers_d5f54ba1d3368c00 = function(arg0, arg1) {
        arg0.buffers = arg1;
    };
    imports.wbg.__wbg_setbytesperrow_7eb4ea50ad336975 = function(arg0, arg1) {
        arg0.bytesPerRow = arg1 >>> 0;
    };
    imports.wbg.__wbg_setclearvalue_1d26e1b07873908a = function(arg0, arg1) {
        arg0.clearValue = arg1;
    };
    imports.wbg.__wbg_setcode_e66de35c80aa100f = function(arg0, arg1, arg2) {
        arg0.code = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setcolor_8d4bfc735001f4bd = function(arg0, arg1) {
        arg0.color = arg1;
    };
    imports.wbg.__wbg_setcolorattachments_6118b962baa6088d = function(arg0, arg1) {
        arg0.colorAttachments = arg1;
    };
    imports.wbg.__wbg_setcompare_019e85bf2bf22bc8 = function(arg0, arg1) {
        arg0.compare = __wbindgen_enum_GpuCompareFunction[arg1];
    };
    imports.wbg.__wbg_setcompare_3a69aad67f43501e = function(arg0, arg1) {
        arg0.compare = __wbindgen_enum_GpuCompareFunction[arg1];
    };
    imports.wbg.__wbg_setcount_2013aa835878f321 = function(arg0, arg1) {
        arg0.count = arg1 >>> 0;
    };
    imports.wbg.__wbg_setcullmode_e82736bddd8d5a5c = function(arg0, arg1) {
        arg0.cullMode = __wbindgen_enum_GpuCullMode[arg1];
    };
    imports.wbg.__wbg_setdepthbias_dc092ae40ce06777 = function(arg0, arg1) {
        arg0.depthBias = arg1;
    };
    imports.wbg.__wbg_setdepthbiasclamp_30724e55c04b7132 = function(arg0, arg1) {
        arg0.depthBiasClamp = arg1;
    };
    imports.wbg.__wbg_setdepthbiasslopescale_3047f42a19dd1d21 = function(arg0, arg1) {
        arg0.depthBiasSlopeScale = arg1;
    };
    imports.wbg.__wbg_setdepthclearvalue_e09b29c35f439d38 = function(arg0, arg1) {
        arg0.depthClearValue = arg1;
    };
    imports.wbg.__wbg_setdepthcompare_7ff390bcd4cbc798 = function(arg0, arg1) {
        arg0.depthCompare = __wbindgen_enum_GpuCompareFunction[arg1];
    };
    imports.wbg.__wbg_setdepthfailop_32e5a25f8472872a = function(arg0, arg1) {
        arg0.depthFailOp = __wbindgen_enum_GpuStencilOperation[arg1];
    };
    imports.wbg.__wbg_setdepthloadop_5292e3e4542c7770 = function(arg0, arg1) {
        arg0.depthLoadOp = __wbindgen_enum_GpuLoadOp[arg1];
    };
    imports.wbg.__wbg_setdepthorarraylayers_57e35a31ded46b97 = function(arg0, arg1) {
        arg0.depthOrArrayLayers = arg1 >>> 0;
    };
    imports.wbg.__wbg_setdepthreadonly_8e4aa6065b3f0cb1 = function(arg0, arg1) {
        arg0.depthReadOnly = arg1 !== 0;
    };
    imports.wbg.__wbg_setdepthstencil_2708265354655cab = function(arg0, arg1) {
        arg0.depthStencil = arg1;
    };
    imports.wbg.__wbg_setdepthstencilattachment_ef75a68ffe787e5a = function(arg0, arg1) {
        arg0.depthStencilAttachment = arg1;
    };
    imports.wbg.__wbg_setdepthstoreop_a7eddf1211b8cf40 = function(arg0, arg1) {
        arg0.depthStoreOp = __wbindgen_enum_GpuStoreOp[arg1];
    };
    imports.wbg.__wbg_setdepthwriteenabled_acc3c3e7425182f8 = function(arg0, arg1) {
        arg0.depthWriteEnabled = arg1 !== 0;
    };
    imports.wbg.__wbg_setdevice_44b06c4615b5e253 = function(arg0, arg1) {
        arg0.device = arg1;
    };
    imports.wbg.__wbg_setdimension_1e40af745768ac00 = function(arg0, arg1) {
        arg0.dimension = __wbindgen_enum_GpuTextureDimension[arg1];
    };
    imports.wbg.__wbg_setdimension_8523a7df804e7839 = function(arg0, arg1) {
        arg0.dimension = __wbindgen_enum_GpuTextureViewDimension[arg1];
    };
    imports.wbg.__wbg_setdstfactor_f1f99957519ecc26 = function(arg0, arg1) {
        arg0.dstFactor = __wbindgen_enum_GpuBlendFactor[arg1];
    };
    imports.wbg.__wbg_setendofpasswriteindex_7e0b2037985d92b3 = function(arg0, arg1) {
        arg0.endOfPassWriteIndex = arg1 >>> 0;
    };
    imports.wbg.__wbg_setentries_5ebe60dce5e74a0b = function(arg0, arg1) {
        arg0.entries = arg1;
    };
    imports.wbg.__wbg_setentries_9e330e1730f04662 = function(arg0, arg1) {
        arg0.entries = arg1;
    };
    imports.wbg.__wbg_setentrypoint_0a1a32e09949ab1d = function(arg0, arg1, arg2) {
        arg0.entryPoint = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setentrypoint_f8a6dd312fc366f9 = function(arg0, arg1, arg2) {
        arg0.entryPoint = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setexternaltexture_c45a65eda8f1c7e7 = function(arg0, arg1) {
        arg0.externalTexture = arg1;
    };
    imports.wbg.__wbg_setfailop_30e3f1483250eade = function(arg0, arg1) {
        arg0.failOp = __wbindgen_enum_GpuStencilOperation[arg1];
    };
    imports.wbg.__wbg_setformat_071b082598e71ae2 = function(arg0, arg1) {
        arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
    };
    imports.wbg.__wbg_setformat_2a57c4eddb717f46 = function(arg0, arg1) {
        arg0.format = __wbindgen_enum_GpuVertexFormat[arg1];
    };
    imports.wbg.__wbg_setformat_45c59d08eefdcb12 = function(arg0, arg1) {
        arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
    };
    imports.wbg.__wbg_setformat_71f884d31aabe541 = function(arg0, arg1) {
        arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
    };
    imports.wbg.__wbg_setformat_726ed8f81a287fdc = function(arg0, arg1) {
        arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
    };
    imports.wbg.__wbg_setformat_8530b9d25ea51775 = function(arg0, arg1) {
        arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
    };
    imports.wbg.__wbg_setformat_d5c08abcb3a02a26 = function(arg0, arg1) {
        arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
    };
    imports.wbg.__wbg_setfragment_a6d6aa2f648896c5 = function(arg0, arg1) {
        arg0.fragment = arg1;
    };
    imports.wbg.__wbg_setfrontface_fccdd9171df26b56 = function(arg0, arg1) {
        arg0.frontFace = __wbindgen_enum_GpuFrontFace[arg1];
    };
    imports.wbg.__wbg_setg_d7b95d11c12af1cb = function(arg0, arg1) {
        arg0.g = arg1;
    };
    imports.wbg.__wbg_sethasdynamicoffset_dcbae080558be467 = function(arg0, arg1) {
        arg0.hasDynamicOffset = arg1 !== 0;
    };
    imports.wbg.__wbg_setheight_28e79506f626af82 = function(arg0, arg1) {
        arg0.height = arg1 >>> 0;
    };
    imports.wbg.__wbg_setheight_2b7714c57b2b4597 = function(arg0, arg1) {
        arg0.height = arg1 >>> 0;
    };
    imports.wbg.__wbg_setheight_4fce583024b2d088 = function(arg0, arg1) {
        arg0.height = arg1 >>> 0;
    };
    imports.wbg.__wbg_setlabel_1183ccaccddf4c32 = function(arg0, arg1, arg2) {
        arg0.label = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setlabel_3d8a20f328073061 = function(arg0, arg1, arg2) {
        arg0.label = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setlabel_53b47ffdebccf638 = function(arg0, arg1, arg2) {
        arg0.label = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setlabel_6d317656a2b3dea6 = function(arg0, arg1, arg2) {
        arg0.label = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setlabel_7ffda3ed69c72b85 = function(arg0, arg1, arg2) {
        arg0.label = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setlabel_828e6fe16c83ad61 = function(arg0, arg1, arg2) {
        arg0.label = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setlabel_95bae3d54f33d3c6 = function(arg0, arg1, arg2) {
        arg0.label = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setlabel_a1c8caea9f6c17d7 = function(arg0, arg1, arg2) {
        arg0.label = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setlabel_a3e682ef8c10c947 = function(arg0, arg1, arg2) {
        arg0.label = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setlabel_c7426807cb0ab0d7 = function(arg0, arg1, arg2) {
        arg0.label = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setlabel_c880c612e67bf9d9 = function(arg0, arg1, arg2) {
        arg0.label = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setlabel_d5ff85faa53a8c67 = function(arg0, arg1, arg2) {
        arg0.label = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setlabel_eb73d9dd282c005a = function(arg0, arg1, arg2) {
        arg0.label = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_setlayout_38ee34b009072f0c = function(arg0, arg1) {
        arg0.layout = arg1;
    };
    imports.wbg.__wbg_setlayout_a9aebce493b15bfb = function(arg0, arg1) {
        arg0.layout = arg1;
    };
    imports.wbg.__wbg_setloadop_15883d29f266b084 = function(arg0, arg1) {
        arg0.loadOp = __wbindgen_enum_GpuLoadOp[arg1];
    };
    imports.wbg.__wbg_setlodmaxclamp_f1429df82c4b3ea8 = function(arg0, arg1) {
        arg0.lodMaxClamp = arg1;
    };
    imports.wbg.__wbg_setlodminclamp_9609dff5684c3fe5 = function(arg0, arg1) {
        arg0.lodMinClamp = arg1;
    };
    imports.wbg.__wbg_setmagfilter_b97a014d5bdb96e4 = function(arg0, arg1) {
        arg0.magFilter = __wbindgen_enum_GpuFilterMode[arg1];
    };
    imports.wbg.__wbg_setmappedatcreation_37dd8bbd1a910924 = function(arg0, arg1) {
        arg0.mappedAtCreation = arg1 !== 0;
    };
    imports.wbg.__wbg_setmask_60410c7f40b0fe24 = function(arg0, arg1) {
        arg0.mask = arg1 >>> 0;
    };
    imports.wbg.__wbg_setmaxanisotropy_cae2737696b22ee1 = function(arg0, arg1) {
        arg0.maxAnisotropy = arg1;
    };
    imports.wbg.__wbg_setminbindingsize_f7d3351b78c71fbc = function(arg0, arg1) {
        arg0.minBindingSize = arg1;
    };
    imports.wbg.__wbg_setminfilter_386c520cd285c6b2 = function(arg0, arg1) {
        arg0.minFilter = __wbindgen_enum_GpuFilterMode[arg1];
    };
    imports.wbg.__wbg_setmiplevel_4adfe9f0872d052d = function(arg0, arg1) {
        arg0.mipLevel = arg1 >>> 0;
    };
    imports.wbg.__wbg_setmiplevelcount_3368440f1c3c34b9 = function(arg0, arg1) {
        arg0.mipLevelCount = arg1 >>> 0;
    };
    imports.wbg.__wbg_setmiplevelcount_9de96fe0db85420d = function(arg0, arg1) {
        arg0.mipLevelCount = arg1 >>> 0;
    };
    imports.wbg.__wbg_setmipmapfilter_ba0ff5e3e86bc573 = function(arg0, arg1) {
        arg0.mipmapFilter = __wbindgen_enum_GpuMipmapFilterMode[arg1];
    };
    imports.wbg.__wbg_setmodule_4a8baf88303e8712 = function(arg0, arg1) {
        arg0.module = arg1;
    };
    imports.wbg.__wbg_setmodule_871baa111fc4d61b = function(arg0, arg1) {
        arg0.module = arg1;
    };
    imports.wbg.__wbg_setmultisample_d07e1d64727f8cc6 = function(arg0, arg1) {
        arg0.multisample = arg1;
    };
    imports.wbg.__wbg_setmultisampled_dc1cdd807d0170e1 = function(arg0, arg1) {
        arg0.multisampled = arg1 !== 0;
    };
    imports.wbg.__wbg_setoffset_51eb43b37f1e9525 = function(arg0, arg1) {
        arg0.offset = arg1;
    };
    imports.wbg.__wbg_setoffset_a0d9f31cd1585a78 = function(arg0, arg1) {
        arg0.offset = arg1;
    };
    imports.wbg.__wbg_setoffset_a90a41961b1df9b4 = function(arg0, arg1) {
        arg0.offset = arg1;
    };
    imports.wbg.__wbg_setonmessage_9052f86e36e1d6a4 = function(arg0, arg1) {
        arg0.onmessage = arg1;
    };
    imports.wbg.__wbg_setonuncapturederror_a15ab4f8fa2275bd = function(arg0, arg1) {
        arg0.onuncapturederror = arg1;
    };
    imports.wbg.__wbg_setoperation_2bbceba9621b7980 = function(arg0, arg1) {
        arg0.operation = __wbindgen_enum_GpuBlendOperation[arg1];
    };
    imports.wbg.__wbg_setorigin_154a83d3703121d7 = function(arg0, arg1) {
        arg0.origin = arg1;
    };
    imports.wbg.__wbg_setpassop_57a439a73e0295e2 = function(arg0, arg1) {
        arg0.passOp = __wbindgen_enum_GpuStencilOperation[arg1];
    };
    imports.wbg.__wbg_setpowerpreference_229fffedb859fda8 = function(arg0, arg1) {
        arg0.powerPreference = __wbindgen_enum_GpuPowerPreference[arg1];
    };
    imports.wbg.__wbg_setprimitive_6c50407f92440018 = function(arg0, arg1) {
        arg0.primitive = arg1;
    };
    imports.wbg.__wbg_setqueryset_1f0efa5a49a1b2ad = function(arg0, arg1) {
        arg0.querySet = arg1;
    };
    imports.wbg.__wbg_setr_6ad5c6f67a5f5a57 = function(arg0, arg1) {
        arg0.r = arg1;
    };
    imports.wbg.__wbg_setrequiredfeatures_8135f6ab89e06b58 = function(arg0, arg1) {
        arg0.requiredFeatures = arg1;
    };
    imports.wbg.__wbg_setresolvetarget_95ee5e55e47822ff = function(arg0, arg1) {
        arg0.resolveTarget = arg1;
    };
    imports.wbg.__wbg_setresource_97233a9ead07e4bc = function(arg0, arg1) {
        arg0.resource = arg1;
    };
    imports.wbg.__wbg_setrowsperimage_b2e56467282d270a = function(arg0, arg1) {
        arg0.rowsPerImage = arg1 >>> 0;
    };
    imports.wbg.__wbg_setsamplecount_df26d31cf04a57d8 = function(arg0, arg1) {
        arg0.sampleCount = arg1 >>> 0;
    };
    imports.wbg.__wbg_setsampler_43a3dd77c3b0a5ba = function(arg0, arg1) {
        arg0.sampler = arg1;
    };
    imports.wbg.__wbg_setsampletype_5671a405c6474494 = function(arg0, arg1) {
        arg0.sampleType = __wbindgen_enum_GpuTextureSampleType[arg1];
    };
    imports.wbg.__wbg_setshaderlocation_99975e71b887d57f = function(arg0, arg1) {
        arg0.shaderLocation = arg1 >>> 0;
    };
    imports.wbg.__wbg_setsize_1a3d1e3a2e547ec1 = function(arg0, arg1) {
        arg0.size = arg1;
    };
    imports.wbg.__wbg_setsize_a45dd219534f95ed = function(arg0, arg1) {
        arg0.size = arg1;
    };
    imports.wbg.__wbg_setsize_e0576eacd9f11fed = function(arg0, arg1) {
        arg0.size = arg1;
    };
    imports.wbg.__wbg_setsrcfactor_368c2472010737bf = function(arg0, arg1) {
        arg0.srcFactor = __wbindgen_enum_GpuBlendFactor[arg1];
    };
    imports.wbg.__wbg_setstencilback_c70185d4a7d8b41f = function(arg0, arg1) {
        arg0.stencilBack = arg1;
    };
    imports.wbg.__wbg_setstencilclearvalue_1580738072a672c0 = function(arg0, arg1) {
        arg0.stencilClearValue = arg1 >>> 0;
    };
    imports.wbg.__wbg_setstencilfront_dc4230c3548ea7f6 = function(arg0, arg1) {
        arg0.stencilFront = arg1;
    };
    imports.wbg.__wbg_setstencilloadop_8486231257ee81bf = function(arg0, arg1) {
        arg0.stencilLoadOp = __wbindgen_enum_GpuLoadOp[arg1];
    };
    imports.wbg.__wbg_setstencilreadmask_027558153bfc424b = function(arg0, arg1) {
        arg0.stencilReadMask = arg1 >>> 0;
    };
    imports.wbg.__wbg_setstencilreadonly_3f415ad876ffa592 = function(arg0, arg1) {
        arg0.stencilReadOnly = arg1 !== 0;
    };
    imports.wbg.__wbg_setstencilstoreop_39fcdf3cc001e427 = function(arg0, arg1) {
        arg0.stencilStoreOp = __wbindgen_enum_GpuStoreOp[arg1];
    };
    imports.wbg.__wbg_setstencilwritemask_6018d5b786f024b1 = function(arg0, arg1) {
        arg0.stencilWriteMask = arg1 >>> 0;
    };
    imports.wbg.__wbg_setstepmode_3b73fd4c54248ad9 = function(arg0, arg1) {
        arg0.stepMode = __wbindgen_enum_GpuVertexStepMode[arg1];
    };
    imports.wbg.__wbg_setstoragetexture_4853479f6eb61a57 = function(arg0, arg1) {
        arg0.storageTexture = arg1;
    };
    imports.wbg.__wbg_setstoreop_0e46dbc6c9712fbb = function(arg0, arg1) {
        arg0.storeOp = __wbindgen_enum_GpuStoreOp[arg1];
    };
    imports.wbg.__wbg_setstripindexformat_be4689e628d10d25 = function(arg0, arg1) {
        arg0.stripIndexFormat = __wbindgen_enum_GpuIndexFormat[arg1];
    };
    imports.wbg.__wbg_settargets_c52d21117ec2cbc0 = function(arg0, arg1) {
        arg0.targets = arg1;
    };
    imports.wbg.__wbg_settexture_5f219a723eb7db43 = function(arg0, arg1) {
        arg0.texture = arg1;
    };
    imports.wbg.__wbg_settexture_84c4ac5434a9ddb5 = function(arg0, arg1) {
        arg0.texture = arg1;
    };
    imports.wbg.__wbg_settimestampwrites_9c3e9dd8a3e800a1 = function(arg0, arg1) {
        arg0.timestampWrites = arg1;
    };
    imports.wbg.__wbg_settopology_0c9fa83132042031 = function(arg0, arg1) {
        arg0.topology = __wbindgen_enum_GpuPrimitiveTopology[arg1];
    };
    imports.wbg.__wbg_settype_0a9fcee42b714ba8 = function(arg0, arg1) {
        arg0.type = __wbindgen_enum_GpuBufferBindingType[arg1];
    };
    imports.wbg.__wbg_settype_298968e371b58a33 = function(arg0, arg1, arg2) {
        arg0.type = getStringFromWasm0(arg1, arg2);
    };
    imports.wbg.__wbg_settype_ba111b7f1813a222 = function(arg0, arg1) {
        arg0.type = __wbindgen_enum_GpuSamplerBindingType[arg1];
    };
    imports.wbg.__wbg_setunclippeddepth_b8bfc6ba4e566a5f = function(arg0, arg1) {
        arg0.unclippedDepth = arg1 !== 0;
    };
    imports.wbg.__wbg_setusage_0f3970011718ab12 = function(arg0, arg1) {
        arg0.usage = arg1 >>> 0;
    };
    imports.wbg.__wbg_setusage_49bed7c9b47e7849 = function(arg0, arg1) {
        arg0.usage = arg1 >>> 0;
    };
    imports.wbg.__wbg_setusage_7ffa4257ea250d02 = function(arg0, arg1) {
        arg0.usage = arg1 >>> 0;
    };
    imports.wbg.__wbg_setusage_8a5ac4564d826d9d = function(arg0, arg1) {
        arg0.usage = arg1 >>> 0;
    };
    imports.wbg.__wbg_setvertex_725cd211418aeffb = function(arg0, arg1) {
        arg0.vertex = arg1;
    };
    imports.wbg.__wbg_setview_2ae2d88e6d071b88 = function(arg0, arg1) {
        arg0.view = arg1;
    };
    imports.wbg.__wbg_setview_5db167adcc0d1b9c = function(arg0, arg1) {
        arg0.view = arg1;
    };
    imports.wbg.__wbg_setviewdimension_2e3a58d96671f97a = function(arg0, arg1) {
        arg0.viewDimension = __wbindgen_enum_GpuTextureViewDimension[arg1];
    };
    imports.wbg.__wbg_setviewdimension_88c1a47ce71f7839 = function(arg0, arg1) {
        arg0.viewDimension = __wbindgen_enum_GpuTextureViewDimension[arg1];
    };
    imports.wbg.__wbg_setviewformats_dbd4d0d50ed403ff = function(arg0, arg1) {
        arg0.viewFormats = arg1;
    };
    imports.wbg.__wbg_setviewformats_e21a9630b45aff68 = function(arg0, arg1) {
        arg0.viewFormats = arg1;
    };
    imports.wbg.__wbg_setvisibility_f4f66940005e5c39 = function(arg0, arg1) {
        arg0.visibility = arg1 >>> 0;
    };
    imports.wbg.__wbg_setwidth_40a6ed203b92839d = function(arg0, arg1) {
        arg0.width = arg1 >>> 0;
    };
    imports.wbg.__wbg_setwidth_503fa5eeec717db8 = function(arg0, arg1) {
        arg0.width = arg1 >>> 0;
    };
    imports.wbg.__wbg_setwidth_64c5783b064042bc = function(arg0, arg1) {
        arg0.width = arg1 >>> 0;
    };
    imports.wbg.__wbg_setwritemask_4198f874c5422156 = function(arg0, arg1) {
        arg0.writeMask = arg1 >>> 0;
    };
    imports.wbg.__wbg_setx_d5236bf9391eb053 = function(arg0, arg1) {
        arg0.x = arg1 >>> 0;
    };
    imports.wbg.__wbg_sety_413262ade3cc0d56 = function(arg0, arg1) {
        arg0.y = arg1 >>> 0;
    };
    imports.wbg.__wbg_setz_a136ba9bd16085f0 = function(arg0, arg1) {
        arg0.z = arg1 >>> 0;
    };
    imports.wbg.__wbg_shiftKey_7793232603bd5f81 = function(arg0) {
        const ret = arg0.shiftKey;
        return ret;
    };
    imports.wbg.__wbg_shiftKey_cf32e1142cac9fca = function(arg0) {
        const ret = arg0.shiftKey;
        return ret;
    };
    imports.wbg.__wbg_signal_da4d466ce86118b5 = function(arg0) {
        const ret = arg0.signal;
        return ret;
    };
    imports.wbg.__wbg_stack_0ed75d68575b0f3c = function(arg0, arg1) {
        const ret = arg1.stack;
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg_start_28944199627e705c = function(arg0) {
        arg0.start();
    };
    imports.wbg.__wbg_static_accessor_GLOBAL_8921f820c2ce3f12 = function() {
        const ret = typeof global === 'undefined' ? null : global;
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    imports.wbg.__wbg_static_accessor_GLOBAL_THIS_f0a4409105898184 = function() {
        const ret = typeof globalThis === 'undefined' ? null : globalThis;
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    imports.wbg.__wbg_static_accessor_SELF_995b214ae681ff99 = function() {
        const ret = typeof self === 'undefined' ? null : self;
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    imports.wbg.__wbg_static_accessor_WINDOW_cde3890479c675ea = function() {
        const ret = typeof window === 'undefined' ? null : window;
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    imports.wbg.__wbg_style_32a3c8393b46a115 = function(arg0) {
        const ret = arg0.style;
        return ret;
    };
    imports.wbg.__wbg_submit_068b03683463d934 = function(arg0, arg1) {
        arg0.submit(arg1);
    };
    imports.wbg.__wbg_then_b33a773d723afa3e = function(arg0, arg1, arg2) {
        const ret = arg0.then(arg1, arg2);
        return ret;
    };
    imports.wbg.__wbg_then_e22500defe16819f = function(arg0, arg1) {
        const ret = arg0.then(arg1);
        return ret;
    };
    imports.wbg.__wbg_unmap_619e40c09473aed0 = function(arg0) {
        arg0.unmap();
    };
    imports.wbg.__wbg_unobserve_189df9b98a2bf3d9 = function(arg0, arg1) {
        arg0.unobserve(arg1);
    };
    imports.wbg.__wbg_userAgentData_f7b0e61c05c54315 = function(arg0) {
        const ret = arg0.userAgentData;
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    imports.wbg.__wbg_userAgent_2e89808dc5dc17d7 = function() { return handleError(function (arg0, arg1) {
        const ret = arg1.userAgent;
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    }, arguments) };
    imports.wbg.__wbg_valueOf_7aa1f7f32182102e = function(arg0) {
        const ret = arg0.valueOf();
        return ret;
    };
    imports.wbg.__wbg_visibilityState_06994c9580901647 = function(arg0) {
        const ret = arg0.visibilityState;
        return (__wbindgen_enum_VisibilityState.indexOf(ret) + 1 || 3) - 1;
    };
    imports.wbg.__wbg_warn_e2ada06313f92f09 = function(arg0) {
        console.warn(arg0);
    };
    imports.wbg.__wbg_wbindgencbdrop_eb10308566512b88 = function(arg0) {
        const obj = arg0.original;
        if (obj.cnt-- == 1) {
            obj.a = 0;
            return true;
        }
        const ret = false;
        return ret;
    };
    imports.wbg.__wbg_wbindgendebugstring_99ef257a3ddda34d = function(arg0, arg1) {
        const ret = debugString(arg1);
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg_wbindgenisfunction_8cee7dce3725ae74 = function(arg0) {
        const ret = typeof(arg0) === 'function';
        return ret;
    };
    imports.wbg.__wbg_wbindgenisnull_f3037694abe4d97a = function(arg0) {
        const ret = arg0 === null;
        return ret;
    };
    imports.wbg.__wbg_wbindgenisobject_307a53c6bd97fbf8 = function(arg0) {
        const val = arg0;
        const ret = typeof(val) === 'object' && val !== null;
        return ret;
    };
    imports.wbg.__wbg_wbindgenisundefined_c4b71d073b92f3c5 = function(arg0) {
        const ret = arg0 === undefined;
        return ret;
    };
    imports.wbg.__wbg_wbindgenstringget_0f16a6ddddef376f = function(arg0, arg1) {
        const obj = arg1;
        const ret = typeof(obj) === 'string' ? obj : undefined;
        var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg_wbindgenthrow_451ec1a8469d7eb6 = function(arg0, arg1) {
        throw new Error(getStringFromWasm0(arg0, arg1));
    };
    imports.wbg.__wbg_webkitFullscreenElement_a9ca38b7214d1567 = function(arg0) {
        const ret = arg0.webkitFullscreenElement;
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    imports.wbg.__wbg_webkitRequestFullscreen_23664c63833ff0e5 = function(arg0) {
        arg0.webkitRequestFullscreen();
    };
    imports.wbg.__wbg_width_092500fcef82abcd = function(arg0) {
        const ret = arg0.width;
        return ret;
    };
    imports.wbg.__wbg_writeBuffer_b479dd5b90cd43eb = function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
        arg0.writeBuffer(arg1, arg2, arg3, arg4, arg5);
    }, arguments) };
    imports.wbg.__wbg_writeTexture_c70826cc2ae8e127 = function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
        arg0.writeTexture(arg1, arg2, arg3, arg4);
    }, arguments) };
    imports.wbg.__wbindgen_cast_042a4c3c7df753ba = function(arg0, arg1) {
        // Cast intrinsic for `Closure(Closure { dtor_idx: 1006, function: Function { arguments: [Externref], shim_idx: 1007, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
        const ret = makeMutClosure(arg0, arg1, 1006, __wbg_adapter_34);
        return ret;
    };
    imports.wbg.__wbindgen_cast_2241b6af4c4b2941 = function(arg0, arg1) {
        // Cast intrinsic for `Ref(String) -> Externref`.
        const ret = getStringFromWasm0(arg0, arg1);
        return ret;
    };
    imports.wbg.__wbindgen_cast_47db4710b4ee7a64 = function(arg0, arg1) {
        // Cast intrinsic for `Closure(Closure { dtor_idx: 572, function: Function { arguments: [NamedExternref("FocusEvent")], shim_idx: 575, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
        const ret = makeMutClosure(arg0, arg1, 572, __wbg_adapter_6);
        return ret;
    };
    imports.wbg.__wbindgen_cast_5e33ae377f2daabc = function(arg0, arg1) {
        // Cast intrinsic for `Closure(Closure { dtor_idx: 572, function: Function { arguments: [NamedExternref("KeyboardEvent")], shim_idx: 575, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
        const ret = makeMutClosure(arg0, arg1, 572, __wbg_adapter_6);
        return ret;
    };
    imports.wbg.__wbindgen_cast_7774e70e4ceac7bf = function(arg0, arg1) {
        // Cast intrinsic for `Closure(Closure { dtor_idx: 572, function: Function { arguments: [NamedExternref("WheelEvent")], shim_idx: 575, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
        const ret = makeMutClosure(arg0, arg1, 572, __wbg_adapter_6);
        return ret;
    };
    imports.wbg.__wbindgen_cast_78c267ec85c0a60f = function(arg0, arg1) {
        // Cast intrinsic for `Closure(Closure { dtor_idx: 989, function: Function { arguments: [NamedExternref("GPUUncapturedErrorEvent")], shim_idx: 990, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
        const ret = makeMutClosure(arg0, arg1, 989, __wbg_adapter_9);
        return ret;
    };
    imports.wbg.__wbindgen_cast_78e170afd483426f = function(arg0, arg1) {
        // Cast intrinsic for `Closure(Closure { dtor_idx: 572, function: Function { arguments: [NamedExternref("Event")], shim_idx: 575, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
        const ret = makeMutClosure(arg0, arg1, 572, __wbg_adapter_6);
        return ret;
    };
    imports.wbg.__wbindgen_cast_7b02d3997b827936 = function(arg0, arg1) {
        // Cast intrinsic for `Closure(Closure { dtor_idx: 572, function: Function { arguments: [NamedExternref("PointerEvent")], shim_idx: 575, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
        const ret = makeMutClosure(arg0, arg1, 572, __wbg_adapter_6);
        return ret;
    };
    imports.wbg.__wbindgen_cast_cb9088102bce6b30 = function(arg0, arg1) {
        // Cast intrinsic for `Ref(Slice(U8)) -> NamedExternref("Uint8Array")`.
        const ret = getArrayU8FromWasm0(arg0, arg1);
        return ret;
    };
    imports.wbg.__wbindgen_cast_d2fd6b40df854d52 = function(arg0, arg1) {
        // Cast intrinsic for `Closure(Closure { dtor_idx: 572, function: Function { arguments: [NamedExternref("Array<any>")], shim_idx: 575, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
        const ret = makeMutClosure(arg0, arg1, 572, __wbg_adapter_6);
        return ret;
    };
    imports.wbg.__wbindgen_cast_d6cd19b81560fd6e = function(arg0) {
        // Cast intrinsic for `F64 -> Externref`.
        const ret = arg0;
        return ret;
    };
    imports.wbg.__wbindgen_cast_e58b18bc8b1d1639 = function(arg0, arg1) {
        // Cast intrinsic for `Closure(Closure { dtor_idx: 572, function: Function { arguments: [NamedExternref("Array<any>"), NamedExternref("ResizeObserver")], shim_idx: 573, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
        const ret = makeMutClosure(arg0, arg1, 572, __wbg_adapter_12);
        return ret;
    };
    imports.wbg.__wbindgen_cast_ebf1562b291bcca6 = function(arg0, arg1) {
        // Cast intrinsic for `Closure(Closure { dtor_idx: 572, function: Function { arguments: [NamedExternref("PageTransitionEvent")], shim_idx: 575, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
        const ret = makeMutClosure(arg0, arg1, 572, __wbg_adapter_6);
        return ret;
    };
    imports.wbg.__wbindgen_cast_f555608acafc5a13 = function(arg0, arg1) {
        // Cast intrinsic for `Closure(Closure { dtor_idx: 572, function: Function { arguments: [], shim_idx: 582, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
        const ret = makeMutClosure(arg0, arg1, 572, __wbg_adapter_31);
        return ret;
    };
    imports.wbg.__wbindgen_init_externref_table = function() {
        const table = wasm.__wbindgen_export_1;
        const offset = table.grow(4);
        table.set(0, undefined);
        table.set(offset + 0, undefined);
        table.set(offset + 1, null);
        table.set(offset + 2, true);
        table.set(offset + 3, false);
        ;
    };

    return imports;
}

function __wbg_init_memory(imports, memory) {

}

function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    __wbg_init.__wbindgen_wasm_module = module;
    cachedDataViewMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;


    wasm.__wbindgen_start();
    return wasm;
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (typeof module !== 'undefined') {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();

    __wbg_init_memory(imports);

    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }

    const instance = new WebAssembly.Instance(module, imports);

    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (typeof module_or_path !== 'undefined') {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (typeof module_or_path === 'undefined') {
        module_or_path = new URL('ruinsofatlantis_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    __wbg_init_memory(imports);

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync };
export default __wbg_init;
