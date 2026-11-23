pub(crate) fn unique_named_index(names: &[String], requested: &str) -> Result<usize, &'static str> {
    let mut matches = names
        .iter()
        .enumerate()
        .filter(|(_, name)| name.as_str() == requested);
    let first = matches
        .next()
        .ok_or("Voice-call output device is unavailable")?;
    if matches.next().is_some() {
        return Err("Voice-call output device name is ambiguous");
    }
    Ok(first.0)
}

#[cfg(test)]
mod tests {
    use super::unique_named_index;

    #[test]
    fn virtual_cable_is_selected_instead_of_speakers() {
        let names = vec!["Speakers".to_owned(), "CABLE Input".to_owned()];
        assert_eq!(unique_named_index(&names, "CABLE Input").unwrap(), 1);
    }

    #[test]
    fn missing_or_ambiguous_cable_does_not_fall_back_to_speakers() {
        assert!(unique_named_index(&["Speakers".to_owned()], "CABLE Input").is_err());
        assert!(unique_named_index(
            &["CABLE Input".to_owned(), "CABLE Input".to_owned()],
            "CABLE Input"
        )
        .is_err());
    }
}
