use crate::display::Display;
use rppal::gpio::{Gpio, InputPin, Trigger};

pub(crate) struct ButtonHandler {
    _pins: Vec<InputPin>,
}

impl ButtonHandler {
    pub(crate) fn new(
        gpio: &Gpio,
        up_pin: u8,
        down_pin: u8,
        lcd: Display,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut up = gpio.get(up_pin)?.into_input_pulldown();

        {
            let lcd = lcd.clone();
            up.set_async_interrupt(Trigger::RisingEdge, move |level| {
                eprintln!("Got up interrupt {:?}", level);

                lcd.backlight_off();
            })?;
        }

        let mut down = gpio.get(down_pin)?.into_input_pulldown();

        down.set_async_interrupt(Trigger::RisingEdge, move |level| {
            eprintln!("Got down interrupt {:?}", level);
            lcd.backlight_on();
        })?;

        let handler = Self {
            _pins: vec![up, down],
        };

        Ok(handler)
    }
}
