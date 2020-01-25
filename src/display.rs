use crate::Status;
use pwr_hd44780::Hd44780;

mod font;

pub struct Display<T>
where
    T: Hd44780,
{
    lcd: T,
}

impl Display<pwr_hd44780::DirectLcd> {
    pub(crate) fn new(device: &str, bus: u16) -> Result<Self, Box<dyn std::error::Error>> {
        let lcd_bus = pwr_hd44780::I2CBus::new(device, bus)?;
        let mut lcd = pwr_hd44780::DirectLcd::new(Box::new(lcd_bus), 20, 4)?;
        lcd.clear()?;

        font::setup(&mut lcd)?;

        Ok(Self { lcd })
    }
}

impl<T> Display<T>
where
    T: Hd44780,
{
    pub(crate) fn update_status(
        &mut self,
        status: &Status,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = status.temperature;
        let [first, middle, last] = split_digits(temp);
        font::print_big_char(&mut self.lcd, first, 0, 0)?;
        font::print_big_char(&mut self.lcd, middle, 5, 0)?;
        // Use bottom fill char to approximate a dot
        self.lcd.print_char_at(3, 9, 5)?;
        font::print_big_char(&mut self.lcd, last, 10, 0)?;

        // self.lcd.print_at(0, 10, "F")?;

        self.lcd
            .print_at(0, 15, format!("{:.1}F", status.target_temperature))?;
        self.lcd
            .print_at(1, 15, format!("{:.1}%", status.humidity))?;
        self.lcd
            .print_at(2, 15, chrono::Local::now().format("%l:%M").to_string())?;

        let run_status = if status.running { "On" } else { "Off" };
        self.lcd.print_at(3, 17, format!("{:>3}", run_status))?;

        Ok(())
    }
}

fn split_digits(number: f32) -> [usize; 3] {
    assert!(number > 0.0 && number < 100.0);

    let digits = (number * 10.0).round() as usize;
    [digits / 100, (digits / 10) % 10, (digits % 10)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_digits_test() {
        let [first, middle, last] = split_digits(12.3456);

        assert_eq!(first, 1);
        assert_eq!(middle, 2);
        assert_eq!(last, 3);
    }

    #[test]
    fn split_digits_test2() {
        let [first, middle, last] = split_digits(70.26);

        assert_eq!(first, 7);
        assert_eq!(middle, 0);
        assert_eq!(last, 3);
    }
}
