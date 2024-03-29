//!
//! # Example
//!
//! ```
//!
//! let mut device = SC16IS752::new(SC16IS750_ADDRESS, i2c)?;
//! device.initalise(Channel::A, UartConfig::default().baudrate(9600))?;
//! device.gpio_set_pin_mode(GPIO::GPIO0, PinMode::Output)?;
//! device.flush(Channel::A)?;
//! loop {
//!     device.write_byte(Channel::A, "a".as_bytes()[0])?;
//!     println!("RX A = {:?}", device.read_byte(Channel::A)?);
//!
//!     for byte in b"This is channel A" {
//!         device.write_byte(Channel::A, byte)?;
//!     }
//!     let mut buf_a: Vec<u8> = vec![];
//!     for _ in 0..device.fifo_available_data(Channel::A)? {
//!         match device.read_byte(Channel::A)? {
//!             Some(byte) => buf_a.push(byte),
//!             None => break,
//!         }
//!     }
//!     println!("RX UART A = {}", String::from_utf8_lossy(&buf_a));
//!
//!     thread::sleep(Duration::from_millis(250));
//!     println!("Testing GPIO states");
//!     device.gpio_set_pin_state(GPIO::GPIO0, PinState::High)?;
//!     println!(
//!         "GPIO0 = {:?} (Output)",
//!         device.gpio_get_pin_state(GPIO::GPIO0)?
//!     );
//!     println!("Set GPIO0 to low");
//!     device.gpio_set_pin_state(GPIO::GPIO0, PinState::Low)?;
//!     thread::sleep(Duration::from_millis(250));
//!     println!(
//!         "GPIO0 = {:?} (Output toggled)",
//!         device.gpio_get_pin_state(GPIO::GPIO0)?
//!     );
//! ```

use embedded_hal::i2c::{blocking::I2c, Error};

const CRYSTAL_FREQ: u32 = 1843200;

/// UARTs Channel A (TXA/RXA) and Channel B (TXB/RXB)
#[derive(Debug, Copy, Clone)]
pub enum Channel {
    /// UART A
    A,
    /// UART B
    B,
}
impl core::fmt::Display for Channel {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// GPIO pins 0-7
#[allow(missing_docs)]
#[derive(Debug)]
pub enum GPIO {
    GPIO0,
    GPIO1,
    GPIO2,
    GPIO3,
    GPIO4,
    GPIO5,
    GPIO6,
    GPIO7,
}
impl core::fmt::Display for GPIO {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[allow(missing_docs)]
#[derive(Debug)]
pub enum PinMode {
    Input,
    Output,
}
impl core::fmt::Display for PinMode {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug)]
pub enum PinState {
    Low,
    High,
}
impl core::fmt::Display for PinState {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[allow(non_camel_case_types)]
pub enum InterruptEventTest {
    RECEIVE_LINE_STATUS_ERROR,
    RECEIVE_TIMEOUT_INTERRUPT,
    RHR_INTERRUPT,
    THR_INTERRUPT,
    MODEM_INTERRUPT,
    INPUT_PIN_CHANGE_STATE,
    RECEIVE_XOFF,
    CTS_RTS_CHANGE,
    UNKNOWN,
}
/// Extra Features Control Register options
#[derive()]
pub enum FeaturesRegister {
    /// IrDaFast controls IrDA mode. (SC16IS762 only)
    ///
    /// false -> IrDA SIR, 3⁄16 pulse ratio, data rate up to 115.2 kbit/s
    /// true -> IrDA SIR, 1⁄4 pulse ratio, data rate up to 1.152 Mbit/s
    IrDaFast = 0x80,
    /// Inverts RTS signal in RS-485 mode
    ///
    /// false -> RTS pin = low during transmission and RTS pin = high during reception
    /// true -> RTS = high during transmission and RTS = low during reception
    AutoRs485RTSOutputInversion = 0x20,
    /// Enable the transmitter to control the RTS pin
    ///
    /// false -> transmitter does not control RTS pin
    /// true -> transmitter controls RTS pin
    AutoRs485DirectionControl = 0x10,
    /// Disables transmitter.
    ///
    /// UART does not send serial data out on the transmit pin, but the transmit FIFO will continue to receive data from host until full. Any data in the TSR will be sent out before the transmitter goes into disable state.
    TxDisable = 0x04,
    /// Disables receiver.
    ///
    /// UART will stop receiving data immediately once this is set to true, and any data in the TSR will be sent to the receive FIFO. User is advised not to set this bit during receiving.
    RxDisable = 0x02,
    /// Enable 9-bit/Multidrop mode (RS-485).
    Multidrop = 0x01,
}

#[allow(missing_docs)]
pub enum Parity {
    NoParity,
    Odd,
    Even,
    ForcedParity1,
    ForcedParity0,
}

#[allow(missing_docs)]
pub struct UartConfig {
    baud: u32,
    word_length: u8,
    parity: Parity,
    stop_bit: u8,
}

impl UartConfig {
    /// Holds parameters for each individual UART
    ///
    /// # Example
    /// let config UartConfig::new(9600, 8, Parity::None, 1);
    pub fn new(baud: u32, word_length: u8, parity: Parity, stop_bit: u8) -> Self {
        Self {
            // Maximum baudrate is 115200
            baud,
            // 5, 6, 7 or 8 bits
            word_length,
            // Use Parity enum
            parity,
            // Use 0, 1 or 2 (insert table here)
            stop_bit,
        }
    }
    pub fn baudrate(mut self, baud: u32) -> Self {
        self.baud = baud;
        self
    }
}

impl Default for UartConfig {
    fn default() -> Self {
        Self {
            baud: 115200,
            word_length: 8,
            parity: Parity::NoParity,
            stop_bit: 1,
        }
    }
}

#[derive(Debug)]
pub struct SC16IS752<I2C> {
    address: u8,
    i2c: I2C,
    fifo: [u8; 2],
    peek_flags: [bool; 2],
    peek_buf: [Option<u8>; 2],
}

impl<I2C, E: Error> SC16IS752<I2C>
where
    I2C: I2c<Error = E>,
{
    pub fn new(device_address: u8, i2c: I2C) -> Result<Self, E> {
        let mut address = device_address;
        if !(0x48..=0x57).contains(&device_address) {
            address = device_address >> 1
        }
        Ok(Self {
            address,
            i2c,
            fifo: [0u8; 2],
            peek_flags: [false; 2],
            peek_buf: [None; 2],
        })
    }

    /// Initalises a single UART using UartConfig struct
    pub fn initalise_uart(&mut self, channel: Channel, config: UartConfig) -> Result<(), E> {
        self.fifo_enable(channel, true)?;
        self.set_baudrate(channel, config.baud)?;
        self.set_line(channel, config.word_length, config.parity, config.stop_bit)?;
        Ok(())
    }

    fn read_register(&mut self, channel: Channel, reg_address: u8) -> Result<u8, E> {
        let mut result = [0];
        self.i2c
            .write_read(
                self.address,
                &[reg_address << 3 | (channel as u8) << 1],
                &mut result,
            )
            .and(Ok(result[0]))
    }

    fn write_register(&mut self, channel: Channel, reg_address: u8, payload: u8) -> Result<(), E> {
        self.i2c.write(
            self.address,
            &[reg_address << 3 | (channel as u8) << 1u8, payload],
        )
    }

    fn set_baudrate(&mut self, channel: Channel, baudrate: u32) -> Result<(), E> {
        let prescaler = match self.read_register(channel, 0x04)? {
            0 => 1,
            _ => 4,
        };
        let divisor = (CRYSTAL_FREQ / prescaler as u32) / (baudrate * 16);

        let mut temp_line_control_register = self.read_register(channel, 0x03)?;
        temp_line_control_register |= 0x80;
        self.write_register(channel, 0x03, temp_line_control_register)?;

        self.write_register(channel, 0x00, divisor.try_into().unwrap())?;
        self.write_register(channel, 0x01, (divisor >> 8).try_into().unwrap())?;

        temp_line_control_register &= 0x7F;
        self.write_register(channel, 0x03, temp_line_control_register)?;

        // {
        //     let actual_baudrate = (CRYSTAL_FREQ / prescaler as u32) / (16 * divisor);
        //     let error = (actual_baudrate - baudrate) * 1000 / baudrate;

        //     println!("UART {channel}: Desired baudrate: {baudrate}");
        //     println!("UART {channel}: Calculated divisor: {divisor}");
        //     println!("UART {channel}: Actual baudrate: {actual_baudrate}");
        //     println!("UART {channel}: Baudrate error: {error}");
        // }
        Ok(())
    }

    fn set_line(
        &mut self,
        channel: Channel,
        data_length: u8,
        parity_select: Parity,
        stop_length: u8,
    ) -> Result<(), E> {
        let mut temp_line_control_register: u8 = self.read_register(channel, 0x03)?;
        temp_line_control_register &= 0xC0;
        {
            println!("line_control_register Register: {temp_line_control_register:#04x}");
        }

        match data_length {
            5 => {}
            6 => temp_line_control_register |= 0x01,
            7 => temp_line_control_register |= 0x02,
            _ => temp_line_control_register |= 0x03,
        };
        if stop_length == 2 {
            temp_line_control_register |= 0x04;
        }
        match parity_select {
            Parity::NoParity => {}
            Parity::Odd => temp_line_control_register |= 0x08,
            Parity::Even => temp_line_control_register |= 0x18,
            Parity::ForcedParity1 => temp_line_control_register |= 0x28,
            Parity::ForcedParity0 => temp_line_control_register |= 0x38,
        }
        self.write_register(channel, 0x03, temp_line_control_register)
    }

    /// This register is used to set an I/O pin direction. Bit 0 to bit 7 controls GPIO0 to GPIO7.
    pub fn gpio_set_pin_mode(&mut self, pin_number: GPIO, pin_direction: PinMode) -> Result<(), E> {
        let mut temp_io_direction_register = self.read_register(Channel::A, 0xA)?;
        match pin_direction {
            PinMode::Output => temp_io_direction_register |= 0x01 << (pin_number as u8),
            PinMode::Input => temp_io_direction_register &= !(0x01 << (pin_number as u8)),
        }
        self.write_register(Channel::A, 0xA, temp_io_direction_register)
    }

    pub fn gpio_set_pin_state(&mut self, pin_number: GPIO, pin_state: PinState) -> Result<(), E> {
        let mut temp_io_direction_register = self.read_register(Channel::A, 0x0B)?;
        match pin_state {
            PinState::High => temp_io_direction_register |= 0x01 << (pin_number as u8),
            PinState::Low => temp_io_direction_register &= !(0x01 << (pin_number as u8)),
        }
        self.write_register(Channel::A, 0x0B, temp_io_direction_register)
    }

    pub fn gpio_get_pin_state(&mut self, pin_number: GPIO) -> Result<PinState, E> {
        let temp_iostate = self.read_register(Channel::A, 0x0B)?;

        if (temp_iostate & (0x01 << (pin_number as u8))) == 0 {
            return Ok(PinState::Low);
        }
        Ok(PinState::High)
    }

    pub fn gpio_get_port_state(&mut self) -> Result<u8, E> {
        self.read_register(Channel::A, 0x0B)
    }

    pub fn gpio_set_port_mode(&mut self, port_io: u8) -> Result<(), E> {
        self.write_register(Channel::A, 0x0A, port_io)
    }

    pub fn gpio_set_port_state(&mut self, port_state: u8) -> Result<(), E> {
        self.write_register(Channel::A, 0x0B, port_state)
    }

    pub fn set_pin_interrupt(&mut self, io_interrupt_enable_register: u8) -> Result<(), E> {
        self.write_register(Channel::A, 0x0C, io_interrupt_enable_register)
    }

    pub fn reset_device(&mut self) -> Result<(), E> {
        let mut reg: u8 = self.read_register(Channel::A, 0x0E)?;
        reg |= 0x08;
        self.write_register(Channel::A, 0x0E, reg)
    }

    pub fn modem_pin(&mut self, state: bool) -> Result<(), E> {
        let mut temp_io_control_register = self.read_register(Channel::A, 0x0E)?;

        if state {
            temp_io_control_register |= 0x02;
        } else {
            temp_io_control_register &= 0xFD;
        }

        self.write_register(Channel::A, 0x0E, temp_io_control_register)
    }

    pub fn gpio_latch(&mut self, latch: bool) -> Result<(), E> {
        let mut temp_io_control_register = self.read_register(Channel::A, 0x0E)?;

        if !latch {
            temp_io_control_register &= 0xFE;
        } else {
            temp_io_control_register |= 0x01;
        }

        self.write_register(Channel::A, 0x0E, temp_io_control_register)
    }

    pub fn interrupt_control(
        &mut self,
        channel: Channel,
        interrupt_enable_register: u8,
    ) -> Result<(), E> {
        self.write_register(channel, 0x01, interrupt_enable_register)
    }

    pub fn interrupt_pending_test(&mut self, channel: Channel) -> Result<u8, E> {
        let ipt = self.read_register(channel, 0x02)?;
        Ok(ipt & 0x01)
    }

    pub fn isr(&mut self, channel: Channel) -> Result<InterruptEventTest, E> {
        let mut interrupt_identification_register = self.read_register(channel, 0x02)?;
        // interrupt_identification_register >>= 1;
        interrupt_identification_register &= 0x3E;
        match interrupt_identification_register {
            0x06 => Ok(InterruptEventTest::RECEIVE_LINE_STATUS_ERROR),
            0x0C => Ok(InterruptEventTest::RECEIVE_TIMEOUT_INTERRUPT),
            0x04 => Ok(InterruptEventTest::RHR_INTERRUPT),
            0x02 => Ok(InterruptEventTest::THR_INTERRUPT),
            0x00 => Ok(InterruptEventTest::MODEM_INTERRUPT),
            0x30 => Ok(InterruptEventTest::INPUT_PIN_CHANGE_STATE),
            0x10 => Ok(InterruptEventTest::RECEIVE_XOFF),
            0x20 => Ok(InterruptEventTest::CTS_RTS_CHANGE),
            _ => Ok(InterruptEventTest::UNKNOWN),
        }
    }

    pub fn fifo_enable(&mut self, channel: Channel, state: bool) -> Result<(), E> {
        let mut fifo_control_register = self.read_register(channel, 0x02)?;

        if !state {
            fifo_control_register &= 0xFE;
        } else {
            fifo_control_register |= 0x01;
        }
        self.write_register(channel, 0x02, fifo_control_register)
    }

    pub fn fifo_reset(&mut self, channel: Channel, state: bool) -> Result<(), E> {
        let mut temp_fcr = self.read_register(channel, 0x02)?;

        if !state {
            temp_fcr |= 0x04;
        } else {
            temp_fcr |= 0x02;
        }
        self.write_register(channel, 0x02, temp_fcr)
    }

    pub fn fifo_set_trigger_level(
        &mut self,
        channel: Channel,
        rx_fifo: bool,
        length: u8,
    ) -> Result<(), E> {
        let mut temp_reg = self.read_register(channel, 0x04)?;
        temp_reg |= 0x04;
        self.write_register(channel, 0x04, temp_reg)?; // SET MCR[2] to '1' to use TLR register or trigger level control in FCR
                                                       // register

        temp_reg = self.read_register(channel, 0x02)?;
        self.write_register(channel, 0x02, temp_reg | 0x10)?; // set ERF[4] to '1' to use the  enhanced features

        if !rx_fifo {
            self.write_register(channel, 0x07, length << 4)?; // Tx FIFO trigger level setting
        } else {
            self.write_register(channel, 0x07, length)?; // Rx FIFO Trigger level setting
        }
        self.write_register(channel, 0x02, temp_reg) // restore EFR register
    }

    pub fn fifo_available_data(&mut self, channel: Channel) -> Result<u8, E> {
        // if self.fifo[channel as usize] == 0 {
        self.fifo[channel as usize] = self.read_register(channel, 0x09)?;
        // }
        Ok(self.fifo[channel as usize])
    }

    pub fn fifo_available_space(&mut self, channel: Channel) -> Result<u8, E> {
        self.read_register(channel, 0x08)
    }

    fn write_byte(&mut self, channel: Channel, val: &u8) -> Result<(), E> {
        let mut tmp_line_status_register: u8 = 0;
        while (tmp_line_status_register & 0x20) == 0 {
            tmp_line_status_register = self.read_register(channel, 0x05)?;
        }
        self.write_register(channel, 0x00, *val)
    }

    pub fn write(&mut self, channel: Channel, payload: &[u8]) -> Result<(), E> {
        for byte in payload {
            self.write_byte(channel, byte)?
        }
        Ok(())
    }

    fn read_byte(&mut self, channel: Channel) -> Result<Option<u8>, E> {
        if self.fifo_available_data(channel)? == 0 {
            //println!("No data");
            return Ok(None);
        }
        Ok(Some(self.read_register(channel, 0x00)?))
    }

    pub fn read(&mut self, channel: Channel, quantity: u8) -> Result<Vec<u8>, E> {
        let mut buf_len: u8 = 0;
        let mut buf: Vec<u8> = vec![];
        if quantity > self.fifo_available_data(channel)? {
            buf_len = self.fifo_available_data(channel)?;
        }
        for _ in 0..=buf_len {
            if let Ok(Some(byte)) = self.read_byte(channel) {
                buf.push(byte);
            }
        }
        Ok(buf)
    }

    pub fn read_all(&mut self, channel: Channel) -> Result<Vec<u8>, E> {
        let mut buf: Vec<u8> = vec![];
        for _ in 0..=self.fifo_available_data(channel)? {
            if let Ok(Some(byte)) = self.read_byte(channel) {
                buf.push(byte);
            }
        }
        Ok(buf)
    }

    pub fn enable_features(
        &mut self,
        channel: Channel,
        feature: FeaturesRegister,
        enable: bool,
    ) -> Result<(), E> {
        let mut temp_extra_features_control_register = self.read_register(channel, 0xF)?;

        if !enable {
            temp_extra_features_control_register |= feature as u8;
        } else {
            temp_extra_features_control_register &= !(feature as u8);
        }
        self.write_register(channel, 0xF, temp_extra_features_control_register)
    }

    pub fn ping(&mut self) -> Result<bool, E> {
        self.write_register(Channel::A, 0x07, 0x55)?;

        if self.read_register(Channel::A, 0x07)? != 0x55 {
            return Ok(false);
        }

        self.write_register(Channel::A, 0x07, 0xAA)?;

        if self.read_register(Channel::A, 0x07)? != 0xAA {
            return Ok(false);
        }

        self.write_register(Channel::B, 0x07, 0x55)?;

        if self.read_register(Channel::B, 0x07)? != 0x55 {
            return Ok(false);
        }

        self.write_register(Channel::B, 0x07, 0xAA)?;

        if self.read_register(Channel::B, 0x07)? != 0xAA {
            return Ok(false);
        }

        Ok(true)
    }

    pub fn flush(&mut self, channel: Channel) -> Result<(), E> {
        let mut tmp_line_status_register: u8 = 0;

        while (tmp_line_status_register & 0x20) == 0 {
            tmp_line_status_register = self.read_register(channel, 0x05)?;
        }
        Ok(())
    }

    pub fn peek(&mut self, channel: Channel) -> Result<(), E> {
        if self.peek_flags[channel as usize] {
            self.peek_buf[channel as usize] = self.read_byte(channel)?;

            if self.peek_buf[channel as usize].is_some() {
                self.peek_flags[channel as usize] = true;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
