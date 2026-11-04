use format198x_apple_ii_woz::Woz;
#[test]
#[ignore = "external WOZ2 image; set FORMAT198X_WOZ_IMAGE"]
fn external_image_round_trips_without_losing_any_bytes() {
    let bytes =
        std::fs::read(std::env::var("FORMAT198X_WOZ_IMAGE").expect("set FORMAT198X_WOZ_IMAGE"))
            .expect("read image");
    let woz = Woz::parse(&bytes).expect("WOZ2 image");
    assert_eq!(woz.to_bytes(), bytes);
}
