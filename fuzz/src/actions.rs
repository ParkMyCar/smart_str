//! Various actions we take on a [`CompactString`] and "control" [`String`], asserting invariants
//! along the way.

use arbitrary::Arbitrary;
use compact_str::CompactString;

#[derive(Arbitrary, Debug)]
pub enum Action<'a> {
    /// Push a character onto our strings
    Push(char),
    /// Pop a number of characters off the string
    Pop(u8),
    /// Push a &str onto our strings
    PushStr(&'a str),
    /// Extend our strings with a collection of characters
    ExtendChars(Vec<char>),
    /// Extend our strings with a collection of strings
    ExtendStr(Vec<&'a str>),
    /// Check to make sure a subslice of our strings are the same
    CheckSubslice(u8, u8),
    /// Make both of our strings uppercase
    MakeUppercase,
    /// Replace a range within both strings with the provided `&str`
    ReplaceRange(u8, u8, &'a str),
    /// Reserve space in our string, no-ops if the `CompactString` would have a capacity > 24MB
    Reserve(u16),
    /// Truncate a string to a new, shorter length
    Truncate(u8),
    /// Insert a string at an index
    InsertStr(u8, &'a str),
    /// Insert a character at an index
    Insert(u8, char),
    /// Reduce the length to zero
    Clear,
    /// Split the string at a given position
    SplitOff(u8),
    /// Extract a range
    Drain(u8, u8),
    /// Remove a `char`
    Remove(u8),
    /// First reserve additional memory, then shrink it
    ShrinkTo(u16, u16),
    /// Remove every nth character, and every character over a specific code point
    Retain(u8, char),
    /// Clones each string, and drops the originals
    CloneAndDrop,
    /// Calls into_bytes, validates equality, and converts back into strings
    RoundTripIntoBytes,
    /// Repeat the string to form a new string.
    Repeat(usize),
    /// Zero out the data backing the string.
    Zeroize,
}

impl Action<'_> {
    pub fn perform(self, control: &mut String, compact: &mut CompactString) {
        use Action::*;

        match self {
            Push(c) => {
                control.push(c);
                compact.push(c);

                let og_capacity = compact.capacity();

                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());

                // if we had to reallocate, then we may have inlined the string
                if og_capacity != compact.capacity() {
                    super::assert_properly_allocated(compact, control);
                }
            }
            Pop(count) => {
                (0..count).for_each(|_| {
                    let a = control.pop();
                    let b = compact.pop();
                    assert_eq!(a, b);
                });
                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());
                assert_eq!(control.is_empty(), compact.is_empty());
            }
            PushStr(s) => {
                control.push_str(s);
                compact.push_str(s);

                let og_capacity = compact.capacity();

                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());

                // if we had to reallocate, then we may have inlined the string
                if og_capacity != compact.capacity() {
                    super::assert_properly_allocated(compact, control);
                }
            }
            ExtendChars(chs) => {
                control.extend(chs.iter());
                compact.extend(chs.iter());

                let og_capacity = compact.capacity();

                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());

                // if we had to re-allocate, make sure we inlined the string, if possible
                if og_capacity != compact.capacity() {
                    super::assert_properly_allocated(compact, control);
                }
            }
            ExtendStr(strs) => {
                control.extend(strs.iter().copied());
                compact.extend(strs.iter().copied());

                let og_capacity = compact.capacity();

                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());

                // if we had to re-allocate, make sure we inlined the string, if possible
                if og_capacity != compact.capacity() {
                    super::assert_properly_allocated(compact, control);
                }
            }
            CheckSubslice(a, b) => {
                assert_eq!(control.len(), compact.len());

                // scale a, b to be [0, 1]
                let c = a as f32 / (u8::MAX as f32);
                let d = b as f32 / (u8::MAX as f32);

                // scale c, b to be [0, compact.len()]
                let e = (c * compact.len() as f32) as usize;
                let f = (d * compact.len() as f32) as usize;

                let lower = core::cmp::min(e, f);
                let upper = core::cmp::max(e, f);

                // scale lower and upper by 1 so we're always comparing at least one character
                let lower = core::cmp::min(lower.wrapping_sub(1), lower);
                let upper = core::cmp::min(upper + 1, compact.len());

                let control_slice = &control.as_bytes()[lower..upper];
                let compact_slice = &compact.as_bytes()[lower..upper];

                assert_eq!(control_slice, compact_slice);
            }
            MakeUppercase => {
                control.as_mut_str().make_ascii_uppercase();
                compact.as_mut_str().make_ascii_uppercase();

                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());
            }
            ReplaceRange(start, end, replace_with) => {
                // turn the arbitrary numbers (start, end) into character indices
                let start = to_index(control, start);
                let end = to_index(control, end);
                let (start, end) = (start.min(end), start.max(end));

                // then apply the replacement
                control.replace_range(start..end, replace_with);
                compact.replace_range(start..end, replace_with);

                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());
            }
            Reserve(num_bytes) => {
                // if this would make us larger then 24MB, then no-op
                if (compact.capacity() + num_bytes as usize) > super::TWENTY_FOUR_MIB_AS_BYTES {
                    return;
                }

                let og_capacity = compact.capacity();

                compact.reserve(num_bytes as usize);
                control.reserve(num_bytes as usize);

                // note: CompactString and String grow at different rates, so we can't assert that
                // their capacities are the same, because they might not be

                assert_eq!(compact, control);
                assert_eq!(compact.len(), control.len());

                // if we had to re-allocate, make sure we inlined the string, if possible
                if og_capacity != compact.capacity()
                    && og_capacity + num_bytes as usize <= super::MAX_INLINE_LENGTH
                {
                    assert!(!compact.is_heap_allocated());
                }
            }
            Truncate(new_len) => {
                // turn the arbitrary number `new_len` into character indices
                let new_len = to_index(control, new_len);

                // then truncate the string
                control.truncate(new_len);
                compact.truncate(new_len);

                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());
            }
            InsertStr(idx, s) => {
                // turn the arbitrary number `new_len` into character indices
                let idx = to_index(control, idx);

                // track our original capacity, so we can make sure the string was inlined if we
                // grew and the resulting string was short enough
                let og_capacity = compact.capacity();

                // then insert the string
                control.insert_str(idx, s);
                compact.insert_str(idx, s);

                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());

                // if our capacity changed, then make sure we took the opportunity to inline the
                // string, if it was short enough
                if og_capacity != compact.capacity() {
                    super::assert_properly_allocated(compact, control);
                }
            }
            Insert(idx, ch) => {
                // turn the arbitrary number `new_len` into character indices
                let idx = to_index(control, idx);

                // track our original capacity, so we can make sure the string was inlined if we
                // grew and the resulting string was short enough
                let og_capacity = compact.capacity();

                // then truncate the string
                control.insert(idx, ch);
                compact.insert(idx, ch);

                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());

                // if our capacity changed, then make sure we took the opportunity to inline the
                // string, if it was short enough
                if og_capacity != compact.capacity() {
                    super::assert_properly_allocated(compact, control);
                }
            }
            Clear => {
                control.clear();
                compact.clear();

                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());
            }
            SplitOff(at) => {
                let at = to_index(control, at);

                let compact_capacity = compact.capacity();
                assert_eq!(compact.split_off(at), control.split_off(at));

                // The capacity of the CompactString should not change when using split_off, unless
                // the CompactString is backed by a &'static str, in which case the capacity should
                // be the point at which we split the string.
                if compact.as_static_str().is_some() {
                    assert_eq!(compact.capacity(), at);
                } else {
                    assert_eq!(compact.capacity(), compact_capacity);
                }

                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());
            }
            Drain(start, end) => {
                let start = to_index(control, start);
                let end = to_index(control, end);
                let (start, end) = (start.min(end), start.max(end));

                let compact_capacity = compact.capacity();
                let is_static = compact.as_static_str().is_some();

                let control_drain = control.drain(start..end);
                let compact_drain = compact.drain(start..end);

                assert_eq!(control_drain.as_str(), compact_drain.as_str());
                drop(control_drain);
                drop(compact_drain);
                assert_eq!(control.as_str(), compact.as_str());

                if !is_static {
                    assert_eq!(compact.capacity(), compact_capacity);
                }
            }
            Remove(val) => {
                let idx = to_index(control, val);

                // idx needs to be < our str length, cycle it back to the beginning if they're equal
                let idx = if idx == control.len() { 0 } else { idx };

                // if the strings are empty, we can't remove anything
                if control.is_empty() && compact.is_empty() {
                    assert_eq!(idx, 0);
                    return;
                }

                assert_eq!(control.remove(idx), compact.remove(idx));
                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());
            }
            ShrinkTo(a, b) => {
                let a = (a % 5000) as usize;
                let b = (b % 5000) as usize;
                let (reserve, shrink) = (a.max(b), a.min(b));

                control.reserve(reserve);
                compact.reserve(reserve);
                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());

                control.shrink_to(shrink);
                compact.shrink_to(shrink);
                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());

                control.shrink_to_fit();
                compact.shrink_to_fit();

                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());
                super::assert_properly_allocated(compact, control);
            }
            Retain(nth, codepoint) => {
                let nth = nth % 8;

                let new_predicate = || {
                    let mut index = 0;
                    move |c: char| {
                        if index == nth || c > codepoint {
                            index = 0;
                            false
                        } else {
                            index += 1;
                            true
                        }
                    }
                };

                control.retain(new_predicate());
                compact.retain(new_predicate());

                assert_eq!(control, compact);
                assert_eq!(control.len(), compact.len());
            }
            CloneAndDrop => {
                let control_clone = control.clone();
                let og = std::mem::replace(control, control_clone);
                drop(og);

                let compact_clone = compact.clone();
                // when cloning, even if the original CompactString was heap allocated, we should
                // inline the new one, if possible
                if compact.len() <= super::MAX_INLINE_LENGTH {
                    assert!(!compact_clone.is_heap_allocated())
                }
                let og = std::mem::replace(compact, compact_clone);
                drop(og);
            }
            RoundTripIntoBytes => {
                // cloning truncates capacity, so we only use the clones for checking equality
                let control_clone = control.clone();
                let compact_clone = compact.clone();

                let og_control = std::mem::take(control);
                let og_compact = std::mem::take(compact);

                let control_bytes = og_control.into_bytes();
                let comapct_bytes = og_compact.into_bytes();

                // check to make sure the bytes are the same!
                assert_eq!(control_bytes.len(), comapct_bytes.len());
                assert_eq!(&control_bytes[..], &comapct_bytes[..]);

                // roundtrip back into strings
                let new_control = String::from_utf8(control_bytes).expect("valid UTF-8");
                let new_compact = CompactString::from_utf8(comapct_bytes).expect("valid UTF-8");

                // make sure we roundtripped successfully
                assert_eq!(new_control, control_clone);
                assert_eq!(new_compact, compact_clone);

                // re-assign
                *control = new_control;
                *compact = new_compact;
            }
            Repeat(times) => {
                // Cap the amount of times we'll repeat to make runs fast.
                let mut times = times % 3;

                // If we'd grow larger than our limit, truncate the string.
                if times * compact.len() > super::TWENTY_FOUR_MIB_AS_BYTES {
                    times = 0;
                }

                let new_compact = compact.repeat(times);
                let new_control = control.repeat(times);

                assert_eq!(new_compact, new_control);
                super::assert_properly_allocated(&new_compact, &new_control);

                *compact = new_compact;
                *control = new_control;
            }
            Zeroize => {
                use zeroize::Zeroize;
                control.zeroize();
                compact.zeroize();
            }
        }
    }
}

fn to_index(s: &str, idx: u8) -> usize {
    s.char_indices()
        .map(|(idx, _)| idx)
        .chain([s.len()])
        .cycle()
        .nth(idx as usize)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::to_index;

    #[test]
    fn test_to_index() {
        let s = "hello world";

        let idx = to_index(s, 5);
        assert_eq!(idx, 5);

        // it should be possible to get str len as an index
        let idx = to_index(s, s.len() as u8);
        assert_eq!(idx, s.len());

        // providing an index greater than the str length, cycles back to the beginning
        let idx = to_index(s, (s.len() + 1) as u8);
        assert_eq!(idx, 0);
    }
}
