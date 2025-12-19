fn main() {
    let uri = "file:///tmp/test.png";
    let digest = md5::compute(uri.as_bytes());
    println!("Hash of '{}': {:x}", uri, digest);
    
    let uri_newline = "file:///tmp/test.png\n";
    let digest_newline = md5::compute(uri_newline.as_bytes());
    println!("Hash of '{}': {:x}", uri_newline.trim(), digest_newline);
}
