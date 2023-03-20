use rusty_ytdl::*;

#[test]
fn generate_random_v6_ip() {
    let ipv6_format = "2001:4::/48";
    println!("{:?}", get_random_v6_ip(ipv6_format).unwrap().to_string());
}
