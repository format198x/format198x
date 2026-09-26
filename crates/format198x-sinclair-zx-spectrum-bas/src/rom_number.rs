//! A number's hidden five-byte form, computed as the 48K ROM computes it.
//!
//! When a line is entered, S-DECIMAL (268D) calls DEC-TO-FP (2C9B), which
//! builds the value digit by digit with the ROM's own calculator, and stores
//! whatever that arithmetic produces. The calculator truncates where exact
//! arithmetic would round, so the stored form is often not the correctly
//! rounded value: `.5` is stored as `7F 7F FF FF FF`, just under a half.
//! This module repeats DEC-TO-FP's steps with the calculator's addition
//! (3014), multiplication (30CA) and division (31AF), bit for bit, so the
//! bytes match what the ROM stores. Addresses are from Logan and O'Hara's
//! *The Complete Spectrum ROM Disassembly*.
//!
//! Every number DEC-TO-FP meets is positive or zero (a minus sign is an
//! operator, not part of the number), so only the positive paths through
//! the arithmetic are reproduced.

/// A number on the calculator stack: either a small integer, `00 sign lo hi
/// 00`, or a full floating-point form, exponent byte then four mantissa
/// bytes with the sign in bit 7 of the first.
type Fp = [u8; 5];

const ZERO: Fp = [0; 5];
/// stk-one (32C8).
const ONE: Fp = [0, 0, 1, 0, 0];
/// stk-ten (32D3).
const TEN: Fp = [0, 0, 10, 0, 0];

/// REPORT-6 (31AD), "Number too big".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Overflow;

/// A small integer, as STACK-BC (2D2B) and INT-STORE (2D8E) store it.
fn small(n: u16) -> Fp {
    let [lo, hi] = n.to_le_bytes();
    [0, 0, lo, hi, 0]
}

fn small_value(x: Fp) -> u32 {
    u32::from(u16::from_le_bytes([x[2], x[3]]))
}

/// The mantissa of a full form with the true numeric bit restored, as
/// PREP-ADD (2F9B) and PREP-M/D (30C0) do; zero for a zero exponent.
fn mantissa(x: Fp) -> u32 {
    if x[0] == 0 {
        return u32::from_be_bytes([x[1], x[2], x[3], x[4]]);
    }
    u32::from_be_bytes([x[1] | 0x80, x[2], x[3], x[4]])
}

/// TEST-ZERO (34E9): the first four bytes are all zero.
fn is_zero(x: Fp) -> bool {
    x[..4].iter().all(|b| *b == 0)
}

/// RE-STACK (3297): a small integer in full floating-point form.
fn re_stack(x: Fp) -> Fp {
    if x[0] != 0 {
        return x;
    }
    let n = u16::from_le_bytes([x[2], x[3]]);
    if n == 0 {
        return ZERO;
    }
    // Up to 16 bits from exponent 0x91, or 8 from 0x89 when the high byte
    // is zero (RS-NRMLSE, 32B1).
    let (mut exponent, mut hl) = if n > 0xFF {
        (0x91u8, n)
    } else {
        (0x89, n << 8)
    };
    // RSTK-LOOP (32B2): shift left until the top bit drops out.
    loop {
        exponent -= 1;
        let carry = hl & 0x8000 != 0;
        hl <<= 1;
        if carry {
            break;
        }
    }
    // Shift back in the (positive) sign bit.
    let [h, l] = (hl >> 1).to_be_bytes();
    [exponent, h, l, 0, 0]
}

/// The result of an operation before OFLOW-CLR (3195) stores it.
struct Result32 {
    exponent: u8,
    /// D'E'DE.
    mantissa: u32,
}

impl Result32 {
    /// OFLOW-CLR (3195): the mantissa, with the positive sign in bit 7.
    fn store(self) -> Fp {
        let [m1, m2, m3, m4] = self.mantissa.to_be_bytes();
        [self.exponent, m1 & 0x7F, m2, m3, m4]
    }
}

/// SKIP-ZERO (315E): 2**-128 when `a` (0x80 for a zero exponent, else 0)
/// meets a normal mantissa, otherwise zero.
fn near_zero(a: u8, mantissa: u32) -> Fp {
    let a = a & mantissa.to_be_bytes()[0];
    // ZEROS-4/5 (2FFB) leaves A as the only mantissa byte; RLCA makes the
    // exponent 1 (for 2**-128) or 0.
    Result32 {
        exponent: a.rotate_left(1),
        mantissa: u32::from(a) << 24,
    }
    .store()
}

/// TEST-NORM (3155) and NORMALISE (316C): shift the mantissa left, taking
/// bits from the fifth byte `a` (rotated circularly, as RLCA does), then
/// round on the next bit of `a`.
fn test_norm(mut exponent: u8, mut mantissa: u32, mut a: u8, carry: bool) -> Result<Fp, Overflow> {
    if carry {
        return Ok(near_zero(if exponent == 0 { 0x80 } else { 0 }, mantissa));
    }
    for _ in 0..32 {
        if mantissa & 0x8000_0000 != 0 {
            // NORML-NOW (3186): round up on the bit below the mantissa.
            if a & 0x80 != 0 {
                mantissa = mantissa.wrapping_add(1);
                if mantissa == 0 {
                    mantissa = 0x8000_0000;
                    exponent = exponent.wrapping_add(1);
                    if exponent == 0 {
                        return Err(Overflow);
                    }
                }
            }
            return Ok(Result32 { exponent, mantissa }.store());
        }
        let bit = a >> 7;
        a = a.rotate_left(1);
        mantissa = (mantissa << 1) | u32::from(bit);
        exponent = exponent.wrapping_sub(1);
        if exponent == 0 {
            return Ok(near_zero(0x80, mantissa));
        }
    }
    Ok(ZERO)
}

/// DIVN-EXPT (313D) to OFLW2-CLR (3151): turn the exponent arithmetic's
/// result `a` (with the S flag it set, and the carry) into the exponent
/// byte, checking for overflow, then normalise.
fn divn_expt(a: u8, sign: bool, carry: bool, mantissa: u32, fifth: u8) -> Result<Fp, Overflow> {
    // RLA, CCF, RRA flip bit 7 and leave the carry as it was.
    let a = a ^ 0x80;
    let mut carry = carry;
    if sign {
        if !carry {
            return Err(Overflow);
        }
        carry = false;
    }
    let exponent = a.wrapping_add(1);
    if exponent == 0 && !carry && mantissa & 0x8000_0000 != 0 {
        return Err(Overflow);
    }
    test_norm(exponent, mantissa, fifth, carry)
}

/// The calculator's addition (3014), `first + second`.
fn add(first: Fp, second: Fp) -> Result<Fp, Overflow> {
    if first[0] | second[0] == 0 {
        let sum = small_value(first) + small_value(second);
        if let Ok(sum) = u16::try_from(sum) {
            return Ok(small(sum));
        }
    }
    // FULL-ADDN (303E).
    let (first, second) = (re_stack(first), re_stack(second));
    // The number with the larger exponent is the augend; on a tie, the
    // second (SHIFT-LEN, 3055).
    let (larger, smaller) = if second[0] >= first[0] {
        (second, first)
    } else {
        (first, second)
    };
    let (augend, mut addend) = (mantissa(larger), mantissa(smaller));
    let mut exponent = larger[0];
    addend = shift_fp(addend, larger[0] - smaller[0]);
    let (mut sum, carry) = augend.overflowing_add(addend);
    if carry {
        // A single shift right brings the carry back in (3073).
        sum = (sum >> 1) | 0x8000_0000;
        let dropped = (augend.wrapping_add(addend)) & 1 != 0;
        if dropped {
            sum = sum.wrapping_add(1);
            if sum == 0 {
                // ADD-BACK rippled right back: ADDEND-0 clears it.
                sum = 0;
            }
        }
        exponent = exponent.wrapping_add(1);
        if exponent == 0 {
            return Err(Overflow);
        }
    }
    test_norm(exponent, sum, 0, false)
}

/// SHIFT-FP (2FDD): shift a positive addend right by `places`, adding back
/// the last bit shifted out; more than 32 places, or a carry that ripples
/// right back, leaves zero.
fn shift_fp(mut n: u32, places: u8) -> u32 {
    if places == 0 {
        return n;
    }
    if places >= 0x21 {
        return 0;
    }
    let mut carry = false;
    for _ in 0..places {
        carry = n & 1 != 0;
        n >>= 1;
    }
    if carry {
        n = n.wrapping_add(1);
    }
    n
}

/// The calculator's multiplication (30CA), `first * second`.
fn multiply(first: Fp, second: Fp) -> Result<Fp, Overflow> {
    if first[0] | second[0] == 0 {
        // HL=HL*DE (30A9) overflows exactly when the product needs 17 bits.
        if let Ok(product) = u16::try_from(small_value(first) * small_value(second)) {
            return Ok(small(product));
        }
    }
    // MULT-LONG (30F0).
    let (first, second) = (re_stack(first), re_stack(second));
    if is_zero(first) {
        return Ok(first);
    }
    if is_zero(second) {
        return Ok(ZERO);
    }
    let multiplicand = mantissa(second);
    // The multiplier in B'C'CA, the result in H'L'HL, both shifted right
    // together 33 times, adding the multiplicand when a 1 drops out
    // (MLT-LOOP 3114, STRT-MLT 3125).
    let mut multiplier = mantissa(first);
    let mut result: u32 = 0;
    let mut carry = false;
    for pass in 0..33 {
        if pass > 0 {
            let mut top = false;
            if carry {
                let (sum, c) = result.overflowing_add(multiplicand);
                result = sum;
                top = c;
            }
            carry = result & 1 != 0;
            result = (result >> 1) | (u32::from(top) << 31);
        }
        let out = multiplier & 1 != 0;
        multiplier = (multiplier >> 1) | (u32::from(carry) << 31);
        carry = out;
    }
    // Add the exponents (3130): ADD A,C, clearing the carry on a zero sum,
    // then DEC A and CCF (MAKE-EXPT, 313B).
    let sum = u16::from(first[0]) + u16::from(second[0]);
    let [a, _] = sum.to_le_bytes();
    let carry = sum > 0xFF && a != 0;
    let a = a.wrapping_sub(1);
    divn_expt(
        a,
        a & 0x80 != 0,
        !carry,
        result,
        multiplier.to_be_bytes()[0],
    )
}

/// The calculator's division (31AF), `first / second`.
fn divide(first: Fp, second: Fp) -> Result<Fp, Overflow> {
    let (first, second) = (re_stack(first), re_stack(second));
    if is_zero(second) {
        return Err(Overflow);
    }
    if is_zero(first) {
        return Ok(first);
    }
    let divisor = mantissa(second);
    let mut rest = mantissa(first);
    // The quotient builds up in B'C'CA; what it held before is shifted out.
    let [m2, m3, m4, _] = mantissa(first).to_be_bytes();
    let mut quotient = u32::from_be_bytes([m2, m3, m4, 0]);
    let mut count: u8 = 0xDF;
    let mut extra = Vec::with_capacity(2);
    let mut carry = false;
    // DIV-START (31E2): a trial subtraction, restoring on a borrow.
    let mut trial = true;
    loop {
        if trial {
            let (difference, borrow) = sbc32(rest, divisor, carry);
            if borrow {
                rest = difference.wrapping_add(divisor);
                carry = false;
            } else {
                rest = difference;
                carry = true;
            }
        }
        // COUNT-ONE (31FA).
        count = count.wrapping_add(1);
        if count & 0x80 == 0 {
            extra.push(carry);
            if count == 0 {
                // The ROM jumps back to DIV-START without shifting the
                // dividend, so the 34th bit is not a true quotient bit.
                trial = true;
                continue;
            }
            break;
        }
        // DIV-LOOP (31D2): the quotient bit in, then the dividend left.
        quotient = (quotient << 1) | u32::from(carry);
        let top = rest & 0x8000_0000 != 0;
        rest <<= 1;
        if top {
            // SUBN-ONLY (31F2): the dropped bit means the divisor goes.
            rest = rest.wrapping_sub(divisor);
            carry = true;
            trial = false;
        } else {
            carry = false;
            trial = true;
        }
    }
    // The 34th and 33rd bits go into B' above its old top six bits.
    let [b, ..] = quotient.to_be_bytes();
    let fifth = (u8::from(extra[0]) << 7) | (u8::from(extra[1]) << 6) | (b >> 2);
    let (a, borrow) = first[0].overflowing_sub(second[0]);
    divn_expt(a, a & 0x80 != 0, borrow, quotient, fifth)
}

/// SBC HL,DE across H'L'HL and D'E'DE: `a - b - carry`, and the borrow.
fn sbc32(a: u32, b: u32, carry: bool) -> (u32, bool) {
    let (d, b1) = a.overflowing_sub(b);
    let (d, b2) = d.overflowing_sub(u32::from(carry));
    (d, b1 || b2)
}

/// A digit's value as STK-DIGIT (2D22) stacks it.
fn digit(d: u8) -> Fp {
    small(u16::from(d - b'0'))
}

/// INT-TO-FP (2D3B): `last = last * 10 + digit` for each digit from `at`,
/// returning the value and where the digits end.
fn int_to_fp(digits: &[u8], mut at: usize) -> Result<(Fp, usize), Overflow> {
    let mut value = ZERO;
    while let Some(&d) = digits.get(at).filter(|d| d.is_ascii_digit()) {
        // exchange, stk-ten, multiply, addition: digit + last * 10.
        value = add(digit(d), multiply(value, TEN)?)?;
        at += 1;
    }
    Ok((value, at))
}

/// E-TO-FP (2D4F): `x * 10**m`, multiplying or dividing by 10, 100, 10**4
/// and so on for each set bit of `|m|`.
fn e_to_fp(mut x: Fp, m: i8) -> Result<Fp, Overflow> {
    let negative = m < 0;
    let mut bits = m.unsigned_abs();
    let mut power = TEN;
    loop {
        let bit = bits & 1 != 0;
        bits >>= 1;
        if bit {
            x = if negative {
                divide(x, power)?
            } else {
                multiply(x, power)?
            };
        }
        if bits == 0 {
            return Ok(x);
        }
        power = multiply(power, power)?;
    }
}

/// DEC-TO-FP (2C9B) for a decimal number's spelling with its spaces
/// removed: digits, an optional point and fraction, an optional exponent.
///
/// # Errors
/// [`Overflow`] where the ROM reports "Number too big": an exponent above
/// 127, or a value the calculator cannot hold.
pub(crate) fn dec_to_fp(spelling: &str) -> Result<[u8; 5], Overflow> {
    let s = spelling.as_bytes();
    let (mut value, mut at) = int_to_fp(s, 0)?;
    if s.get(at) == Some(&b'.') {
        at += 1;
        // DEC-STO-1 (2CD5): mem-0 starts at one and is divided by ten for
        // each digit, which is multiplied by it and added (NXT-DGT-1, 2CDA).
        let mut place = ONE;
        while let Some(&d) = s.get(at).filter(|d| d.is_ascii_digit()) {
            place = divide(place, TEN)?;
            value = add(value, multiply(digit(d), place)?)?;
            at += 1;
        }
    }
    if s.get(at).is_some_and(|b| b.eq_ignore_ascii_case(&b'e')) {
        // SIGN-FLAG (2CF2) and ST-E-PART (2CFF): FP-TO-A reports an
        // exponent above 127.
        at += 1;
        let negative = s.get(at) == Some(&b'-');
        if matches!(s.get(at), Some(b'+' | b'-')) {
            at += 1;
        }
        let mut m: u32 = 0;
        for d in s[at..].iter().take_while(|d| d.is_ascii_digit()) {
            m = (m * 10 + u32::from(d - b'0')).min(1000);
        }
        let m = i8::try_from(m).map_err(|_| Overflow)?;
        value = e_to_fp(value, if negative { -m } else { m })?;
    }
    Ok(value)
}

/// BIN's value, as BIN-END (2CB3) stacks it with STACK-BC.
pub(crate) fn bin_to_fp(value: u16) -> [u8; 5] {
    small(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(spelling: &str) -> String {
        dec_to_fp(spelling)
            .expect("in range")
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn values_the_rom_stores_when_they_are_typed_in() {
        // Captured from the ROM's editor (tests/fixtures/rom-editor).
        assert_eq!(hex(".5"), "7F 7F FF FF FF");
        assert_eq!(hex(".25"), "7E 7F FF FF FF");
        assert_eq!(hex("0.1"), "7D 4C CC CC CC");
        assert_eq!(hex("1.5"), "81 40 00 00 00");
        assert_eq!(hex(".55"), "80 0C CC CC CD");
        assert_eq!(hex("1.5E3"), "8B 3B 80 00 00");
        assert_eq!(hex("1.E3"), "00 00 E8 03 00");
        assert_eq!(hex("12.5E2"), "8B 1C 40 00 00");
        assert_eq!(hex("1.5E-3"), "77 44 9B A5 E3");
        assert_eq!(hex("7"), "00 00 07 00 00");
    }

    #[test]
    fn overflow_is_reported() {
        assert_eq!(dec_to_fp("1E128"), Err(Overflow));
        assert_eq!(dec_to_fp("1E39"), Err(Overflow));
    }
}
