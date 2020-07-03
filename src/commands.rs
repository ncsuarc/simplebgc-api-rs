use std::io;
use std::mem::MaybeUninit;

use bitflags::bitflags;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use duplicate::duplicate;
use impl_trait_for_tuples::impl_for_tuples;

pub trait Command
where
    // TODO: WTF is with this bound
    Self: Sized,
{
    const ID: u8;
    fn parse_payload<R: ReadBytesExt>(reader: &mut R) -> io::Result<Self>;
    fn write_payload<W: WriteBytesExt>(&self, writer: &mut W) -> io::Result<()>;
}

trait Transmit
where
    Self: Sized,
{
    fn validate(&self) -> io::Result<()> {
        Ok(())
    }
    fn from_reader<R: ReadBytesExt>(reader: &mut R) -> io::Result<Self>;
    fn to_writer<W: WriteBytesExt>(&self, writer: &mut W) -> io::Result<()>;
}

// This get's special treatment because it's not generic
impl Transmit for u8 {
    #[inline]
    fn from_reader<R: ReadBytesExt>(reader: &mut R) -> io::Result<Self> {
        reader.read_u8()
    }
    #[inline]
    fn to_writer<W: WriteBytesExt>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_u8(*self)
    }
}

impl Transmit for i8 {
    #[inline]
    fn from_reader<R: ReadBytesExt>(reader: &mut R) -> io::Result<Self> {
        reader.read_i8()
    }
    #[inline]
    fn to_writer<W: WriteBytesExt>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_i8(*self)
    }
}

impl Transmit for bool {
    #[inline]
    fn from_reader<R: ReadBytesExt>(reader: &mut R) -> io::Result<Self> {
        use io::{Error, ErrorKind};
        match reader.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                "bool must be either 0 or 1",
            )),
        }
    }
    #[inline]
    fn to_writer<W: WriteBytesExt>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            false => writer.write_u8(0),
            true => writer.write_u8(1),
        }
    }
}

#[duplicate(
  Num   read_fn    write_fn;
  [u16] [read_u16] [write_u16];
  [i16] [read_i16] [write_i16];
  [u32] [read_u32] [write_u32];
  [i32] [read_i32] [write_i32];
  [u64] [read_u64] [write_u64];
  [i64] [read_i64] [write_i64];
  [f64] [read_f64] [write_f64];
)]
impl Transmit for Num {
    #[inline]
    fn from_reader<R: ReadBytesExt>(reader: &mut R) -> io::Result<Self> {
        reader.read_fn::<LittleEndian>()
    }
    #[inline]
    fn to_writer<W: WriteBytesExt>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_fn::<LittleEndian>(*self)
    }
}

// TODO: Rework me when const_generics land
#[duplicate(
  N; [2]; [3]; [5]; [7]; [9]; [12]; [32];
)]
impl<T: Transmit> Transmit for [T; N] {
    #[inline]
    fn from_reader<R: ReadBytesExt>(reader: &mut R) -> io::Result<Self> {
        // TODO: Is there a safe way to do this?
        // Safety: We initalize the entire array
        let mut data: Self = unsafe { MaybeUninit::uninit().assume_init() };
        for elem in data.iter_mut() {
            *elem = T::from_reader(reader)?;
        }
        Ok(data)
    }
    #[inline]
    fn to_writer<W: WriteBytesExt>(&self, writer: &mut W) -> io::Result<()> {
        for elem in self.iter() {
            elem.to_writer(writer)?;
        }
        Ok(())
    }
}

// TODO: Rework me when variadic generics land
#[impl_for_tuples(5)]
impl Transmit for Tuple {
    #[inline]
    fn from_reader<R: ReadBytesExt>(reader: &mut R) -> io::Result<Self> {
        let mut data: Self = unsafe { MaybeUninit::uninit().assume_init() };
        for_tuples!(#(data.Tuple = Tuple::from_reader(reader)?;)*);
        Ok(data)
    }
    #[inline]
    fn to_writer<W: WriteBytesExt>(&self, writer: &mut W) -> io::Result<()> {
        for_tuples!(#(self.Tuple.to_writer(writer)?;)*);
        Ok(())
    }
}

// TODO: Remove when https://github.com/bitflags/bitflags/pull/220 lands
macro_rules! impl_bflags {
    ($flags:ty, $num:ty) => {
        impl Transmit for $flags {
            #[inline]
            fn from_reader<R: ReadBytesExt>(reader: &mut R) -> io::Result<Self> {
                use std::io::{Error, ErrorKind};
                match <$flags>::from_bits(<$num>::from_reader(reader)?) {
                    Some(bits) => Ok(bits),
                    None => Err(Error::new(ErrorKind::InvalidData, "invalid bits")),
                }
            }
            #[inline]
            fn to_writer<W: WriteBytesExt>(&self, writer: &mut W) -> io::Result<()> {
                self.bits().to_writer(writer)
            }
        }
    };
}

// TODO: Figure out a better way to modularize these

bitflags! {
  struct BoardInfoStateFlags: u8 {
    /// Internal use only.
    const DEBUG_MODE                = 0b00001;
    /// System is re-configured for frame inversion over middle motor
    const IS_FRAME_INVERTED         = 0b00010;
    /// Finished initialization of all basic sensors. Frame inversion configuration is applied.
    const INIT_STEP1_DONE           = 0b00100;
    /// Finished initialization of the RC subsystem, adjustable variables, etc. Automated
    /// positioning is started.
    const INIT_STEP2_DONE           = 0b01000;
    /// Positioning and calibrations at startup is finished.
    const STARTUP_AUTO_ROUTINE_DONE = 0b10000;
  }
}
impl_bflags!(BoardInfoStateFlags, u8);

bitflags! {
  struct BoardInfoFeatures: u16 {
    const THREE_AXIS     = 0b000001;
    const BAT_MONITORING = 0b000010;
    const ENCODERS       = 0b000100;
    const BODE_TEST      = 0b001000;
    const SCRIPTING      = 0b010000;
    const CURRENT_SENSOR = 0b100000;
  }
}
impl_bflags!(BoardInfoFeatures, u16);

bitflags! {
  struct BoardInfoConnectionFlags: u8 {
    const CONNECTION_USB = 0b1;
  }
}
impl_bflags!(BoardInfoConnectionFlags, u8);

// TODO: Figure out a good way to mark this as incoming

#[derive(Command, Transmit)]
#[id(86)]
/// CMD_BOARD_INFO – version and board information
struct BoardInfo {
    /// Unique Id used to identify each controller in licensing system
    board_ver: u8,
    /// Split into decimal digits X.XX.X, for example 2305 means 2.30b5
    firmware_ver: u16,
    state_flags1: BoardInfoStateFlags,
    board_features: BoardInfoFeatures,
    connection_flag: BoardInfoConnectionFlags,
    frw_extra_id: u64,
    _reserved: [u8; 7],
}

#[derive(Command, Transmit)]
#[id(20)]
/// CMD_BOARD_INFO_3 – additional board information
struct BoardInfo3 {
    device_id: [u8; 9],
    mcu_id: [u8; 12],
    eeprom_size: u64,
    // TODO: Is a tuple a good way to represent this?
    script_slot_size: [u16; 5],
    // TODO: Use bitflags for this
    profile_set_slots: u8,
    #[range(1..=6)]
    profile_set_cur: u8,
    _reserved: [u8; 32],
}

#[derive(Transmit)]
struct MotorStatus {
    #[range(0..=255)]
    p: u8,
    #[range(0..=255)]
    i: u8,
    #[range(0..=255)]
    d: u8,
    #[range(0..=255)]
    power: u8,
    invert: bool,
    #[range(0..=255)]
    poles: u8,
}

// TODO: Clean up RcMode handling!!!
enum RcModeControl {
    Angle,
    Speed,
}

struct RcMode {
    mode: RcModeControl,
    inverted: bool,
}

impl Transmit for RcMode {
    fn from_reader<R: ReadBytesExt>(reader: &mut R) -> io::Result<Self> {
        let bits = reader.read_u8()?;
        use io::{Error, ErrorKind};
        let mode = match bits & 0b0000_0011 {
            0b00 => RcModeControl::Angle,
            0b01 => RcModeControl::Speed,
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "lower 2 mode bits can only by 0b00 or 0b01",
                ))
            }
        };
        let inverted = bits & 0b0000_0100 != 0;
        Ok(Self { mode, inverted })
    }

    fn to_writer<W: WriteBytesExt>(&self, writer: &mut W) -> io::Result<()> {
        let mut bits = 0;
        bits |= match self.mode {
            RcModeControl::Angle => 0b00,
            RcModeControl::Speed => 0b01,
        };
        bits |= match self.inverted {
            false => 0b000,
            true => 0b100,
        };
        writer.write_u8(bits)
    }
}

#[derive(Transmit)]
struct RcStatus {
    #[range(-720..=720)]
    min_angle: i16,
    #[range(-720..=720)]
    max_angle: i16,
    mode: RcMode,
    #[range(0..=15)]
    lpf: u8,
    #[range(0..=255)]
    speed: u8,
    // TODO: Read the nodes on this and do some special handling according? Seems difficult
    #[range(-127..=127)]
    follow: i8,
}

#[derive(Transmit)]
#[repr(u8)]
enum PWMFrequency {
    Low = 0,
    High = 1,
    Pitch = 2,
}

#[derive(Transmit)]
#[repr(u8)]
enum BaudRate {
    Baud115200 = 0,
    Baud57600 = 1,
    Baud38400 = 2,
    Baud19200 = 3,
    Baud9600 = 4,
    Baud256000 = 5,
}

#[derive(Command, Transmit)]
#[id(21)]
struct ReadParams3 {
    #[range(0..=4, 255..=255)]
    profile_id: u8,
    axis: [MotorStatus; 3],
    #[range(0..=255)]
    acc_limiter_all: u8,
    ext_fc_gain: [i8; 2],
    rc_status: [RcStatus; 3],
    #[range(0..=255)]
    gyro_thrust: u8,
    use_model: bool,
    pwm_freq: PWMFrequency,
    // TODO: Is this a typo?
    serial_spped: BaudRate,
    // TODO: Ugh, I need ranges on arrays. Probably use iter_range()
}
