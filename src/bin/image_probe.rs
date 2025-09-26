fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("usage: image_probe <png>");
    let img = image::open(&path)?.to_rgba8();
    let (mut rmin, mut gmin, mut bmin) = (255u8, 255u8, 255u8);
    let (mut rmax, mut gmax, mut bmax) = (0u8, 0u8, 0u8);
    for px in img.pixels() {
        let [r, g, b, _a] = px.0;
        rmin = rmin.min(r);
        gmin = gmin.min(g);
        bmin = bmin.min(b);
        rmax = rmax.max(r);
        gmax = gmax.max(g);
        bmax = bmax.max(b);
    }
    println!(
        "min=({},{},{}), max=({},{},{})",
        rmin, gmin, bmin, rmax, gmax, bmax
    );
    Ok(())
}
