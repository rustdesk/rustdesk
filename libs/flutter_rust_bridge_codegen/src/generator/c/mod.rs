pub fn generate_dummy(func_names: &[String]) -> String {
    format!(
        r#"static int64_t dummy_method_to_enforce_bundling(void) {{
    int64_t dummy_var = 0;
{}
    return dummy_var;
}}"#,
        func_names
            .iter()
            .map(|func_name| { format!("    dummy_var ^= ((int64_t) (void*) {});", func_name) })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}
