pub fn get_filter_domains() -> Vec<String> {
    vec![
        "_homekit._tcp.local".into(),
        "_hap._tcp.local".into(),
        "_googlecast._tcp.local".into(),
    ]
}
