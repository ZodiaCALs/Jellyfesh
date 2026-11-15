//! Settings › Playback decisions that are not widgets: the Output Device
//! choices. The core lists the devices and applies the choice.

/// The Output Device choices: System Default (`None`) first, then each
/// device by name, and the position of `selected` among them. A chosen
/// device that is not connected now stays listed (last), so the choice the
/// core keeps is the one shown.
pub fn output_choices(devices: &[String], selected: Option<&str>) -> (Vec<Option<String>>, u32) {
    let mut choices: Vec<Option<String>> = std::iter::once(None)
        .chain(devices.iter().cloned().map(Some))
        .collect();
    let position = match selected {
        None => 0,
        Some(name) => match choices.iter().position(|c| c.as_deref() == Some(name)) {
            Some(i) => i,
            None => {
                choices.push(Some(name.to_owned()));
                choices.len() - 1
            }
        },
    };
    (choices, u32::try_from(position).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn system_default_comes_first() {
        let (choices, at) = output_choices(&names(&["USB DAC", "HDMI"]), None);
        assert_eq!(choices, [None, Some("USB DAC".into()), Some("HDMI".into())]);
        assert_eq!(at, 0);
        let (_, at) = output_choices(&names(&["USB DAC", "HDMI"]), Some("HDMI"));
        assert_eq!(at, 2);
    }

    #[test]
    fn a_missing_choice_stays_listed() {
        let (choices, at) = output_choices(&names(&["HDMI"]), Some("USB DAC"));
        assert_eq!(choices, [None, Some("HDMI".into()), Some("USB DAC".into())]);
        assert_eq!(at, 2);
        let (choices, at) = output_choices(&[], None);
        assert_eq!(choices, [None]);
        assert_eq!(at, 0);
    }
}
