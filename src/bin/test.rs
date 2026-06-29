fn main() {
    let payload = vimit::demo_payload();
    let windows = vimit::summarize_me(&payload, 75.0, 90.0);
    for w in windows {
        println!("{:?}", w);
    }
}
