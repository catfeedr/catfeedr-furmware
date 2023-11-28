use heapless::String;

#[repr(C)]
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct AnimalTag {
    head: u8,
    card_number: [u8; 10],
    country_id: [u8; 4],
    data: u8,
    animal_flag: u8,
    _reserved: [u8; 4],
    user_data: [u8; 6],
    check_sum: [u8; 2],
    tail: u8,
}

impl AnimalTag {
    pub fn card_number(&self) -> String<10> {
        let mut buf = String::<10>::new();

        for hex_num in self.card_number.iter().rev() {
            let _ = buf.push(*hex_num as char);
        }

        buf
    }
}

impl From<[u8; 30]> for AnimalTag {
    fn from(value: [u8; 30]) -> Self {
        let mut tag = AnimalTag {
            head: value[0],
            ..Default::default()
        };

        tag.card_number.copy_from_slice(&value[1..11]);
        tag.country_id.copy_from_slice(&value[11..15]);
        tag.data = value[15];
        tag.animal_flag = value[16];
        tag._reserved.copy_from_slice(&value[17..21]);
        tag.user_data.copy_from_slice(&value[21..27]);
        tag.check_sum = [value[27], value[28]];
        tag.tail = value[29];

        tag
    }
}
