// Adapted from
// https://github.com/gcassarino/BigFont/blob/ee4c39133df1eeed914733f8cb8170e2b440cdae/src/BigFont.h
use pwr_hd44780::Hd44780;

pub(crate) fn setup(lcd: &mut impl Hd44780) -> Result<(), Box<dyn std::error::Error>> {
    lcd.create_char(0, CUSTCHAR3[0])?;
    lcd.create_char(1, CUSTCHAR3[1])?;
    lcd.create_char(2, CUSTCHAR3[2])?;
    lcd.create_char(3, CUSTCHAR3[3])?;
    lcd.create_char(4, CUSTCHAR3[4])?;
    lcd.create_char(5, CUSTCHAR3[5])?;
    lcd.create_char(6, CUSTCHAR3[6])?;
    lcd.create_char(7, CUSTCHAR3[7])?;

    Ok(())
}

pub(crate) fn print_big_char(
    lcd: &mut impl Hd44780,
    digit: usize,
    col: usize,
    row: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // FIXME: This should really return an error
    if digit > 9 {
        return Ok(());
    }

    for i in 0..4 {
        lcd.move_at(row + i, col)?;

        for j in 0..4 {
            lcd.print_char(BIGNUMS[digit][i][j])?;
        }
    }

    Ok(())
}
const BIGNUMS: [[[u8; 4]; 4]; 10] = [
    [
        // 0
        [1, 2, 2, 3],
        [255, 254, 254, 255],
        [255, 254, 254, 255],
        [4, 5, 5, 6],
    ],
    [
        // 1
        [254, 1, 255, 254],
        [254, 254, 255, 254],
        [254, 254, 255, 254],
        [254, 5, 255, 5],
    ],
    [
        // 2
        [1, 2, 2, 3],
        [254, 254, 5, 6],
        [1, 2, 254, 254],
        [255, 5, 5, 5],
    ],
    [
        // 3
        [1, 2, 2, 3],
        [254, 254, 5, 6],
        [254, 254, 2, 3],
        [4, 5, 5, 6],
    ],
    [
        // 4
        [255, 254, 254, 255],
        [255, 254, 254, 255],
        [4, 5, 5, 255],
        [254, 254, 254, 255],
    ],
    [
        // 5
        [255, 2, 2, 2],
        [2, 2, 2, 3],
        [254, 254, 254, 255],
        [4, 5, 5, 6],
    ],
    [
        // 6
        [1, 2, 2, 254],
        [255, 5, 5, 5],
        [255, 254, 254, 255],
        [4, 5, 5, 6],
    ],
    [
        // 7
        [4, 2, 2, 255],
        [254, 254, 5, 6],
        [254, 1, 2, 254],
        [254, 6, 254, 254],
    ],
    [
        // 8
        [1, 2, 2, 3],
        [4, 5, 5, 6],
        [1, 2, 2, 3],
        [4, 5, 5, 6],
    ],
    [
        // 9
        [1, 2, 2, 3],
        [255, 254, 254, 255],
        [4, 5, 5, 255],
        [254, 5, 5, 6],
    ],
];

const CUSTCHAR3: [[u8; 8]; 8] = [
    [
        // 0 // slash used for number 4
        0b00001, 0b00011, 0b00111, 0b01111, 0b11111, 0b11110, 0b11100, 0b11000,
    ],
    [
        // 1
        0b00011, 0b00111, 0b01111, 0b11111, 0b11111, 0b11111, 0b11111, 0b11111,
    ],
    [
        // 2
        0b11111, 0b11111, 0b11111, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000,
    ],
    [
        // 3
        0b11000, 0b11100, 0b11110, 0b11111, 0b11111, 0b11111, 0b11111, 0b11111,
    ],
    [
        // 4
        0b11111, 0b11111, 0b11111, 0b11111, 0b11111, 0b01111, 0b00111, 0b00011,
    ],
    [
        // 5
        0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111, 0b11111, 0b11111,
    ],
    [
        // 6
        0b11111, 0b11111, 0b11111, 0b11111, 0b11111, 0b11110, 0b11100, 0b11000,
    ],
    [
        // 7 (alarm symbol)
        0b00100, 0b01110, 0b01110, 0b01110, 0b11111, 0b00000, 0b00100, 0b00000,
    ],
];
