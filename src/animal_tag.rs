use alloc::format;
use alloc::string::String;

#[repr(C)]
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct AnimalTag {
    head: u8,
    pub card_number: u64,
    country_id: u32,
    data: u8,
    animal_flag: u8,
    _reserved: [u8; 4],
    user_data: [u8; 6],
    check_sum: [u8; 2],
    tail: u8,
}

fn convert_dumb_thing_to_number(dumb_thing: &[u8]) -> u64 {
    // SAFETY: The format is ASCII-based, so reversing the bytes is still valid UTF-8.
    unsafe {
        let mut s = String::from_utf8_unchecked(dumb_thing.to_vec());
        s.as_bytes_mut().reverse();
        u64::from_str_radix(&s, 16).unwrap_or_default()
    }
}

impl AnimalTag {
    pub fn id(&self) -> String {
        format!(
            "{}-{:0width$}",
            self.country_id,
            self.card_number,
            width = 12
        )
    }
}

impl From<[u8; 30]> for AnimalTag {
    fn from(value: [u8; 30]) -> Self {
        let mut tag = AnimalTag {
            head: value[0],
            ..Default::default()
        };

        tag.card_number = convert_dumb_thing_to_number(&value[1..11]);
        tag.country_id = convert_dumb_thing_to_number(&value[11..15]) as u32;
        tag.data = value[15];
        tag.animal_flag = value[16];
        tag._reserved.copy_from_slice(&value[17..21]);
        tag.user_data.copy_from_slice(&value[21..27]);
        tag.check_sum = [value[27], value[28]];
        tag.tail = value[29];

        tag
    }
}
