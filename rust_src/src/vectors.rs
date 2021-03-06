//! Functions operating on vector(like)s, and general sequences.

use std::cmp::Ordering;
use std::mem;
use std::ptr;
use std::slice;

use libc::ptrdiff_t;

use remacs_macros::lisp_fn;
use remacs_sys::{EmacsInt, Lisp_Bool_Vector, Lisp_Vector, Lisp_Vectorlike,
                 Lisp_Vectorlike_With_Slots, PseudovecType, MOST_POSITIVE_FIXNUM,
                 PSEUDOVECTOR_AREA_BITS, PSEUDOVECTOR_FLAG, PSEUDOVECTOR_SIZE_MASK, PVEC_TYPE_MASK};
use remacs_sys::Qsequencep;

use buffers::LispBufferRef;
use chartable::{LispCharTableRef, LispSubCharTableAsciiRef, LispSubCharTableRef};
use data::aref;
use frames::LispFrameRef;
use lisp::{ExternalPtr, LispObject, LispSubrRef};
use lisp::defsubr;
use lists::{inorder, nth, sort_list};
use multibyte::MAX_CHAR;
use process::LispProcessRef;
use threads::ThreadStateRef;
use windows::LispWindowRef;

pub type LispVectorlikeRef = ExternalPtr<Lisp_Vectorlike>;
pub type LispVectorRef = ExternalPtr<Lisp_Vector>;
pub type LispBoolVecRef = ExternalPtr<Lisp_Bool_Vector>;
pub type LispVectorlikeSlotsRef = ExternalPtr<Lisp_Vectorlike_With_Slots>;

impl LispVectorlikeRef {
    #[inline]
    pub fn is_vector(self) -> bool {
        self.header.size & PSEUDOVECTOR_FLAG == 0
    }

    #[inline]
    pub fn as_vector(&self) -> Option<LispVectorRef> {
        if self.is_vector() {
            Some(unsafe { mem::transmute::<_, LispVectorRef>(*self) })
        } else {
            None
        }
    }

    #[inline]
    pub unsafe fn as_vector_unchecked(&self) -> LispVectorRef {
        mem::transmute::<_, LispVectorRef>(*self)
    }

    #[inline]
    pub fn pseudovector_type(self) -> PseudovecType {
        unsafe {
            mem::transmute(((self.header.size & PVEC_TYPE_MASK) >> PSEUDOVECTOR_AREA_BITS) as i32)
        }
    }

    #[inline]
    pub fn is_pseudovector(self, tp: PseudovecType) -> bool {
        self.header.size & (PSEUDOVECTOR_FLAG | PVEC_TYPE_MASK)
            == (PSEUDOVECTOR_FLAG | ((tp as isize) << PSEUDOVECTOR_AREA_BITS))
    }

    #[inline]
    pub fn pseudovector_size(self) -> EmacsInt {
        (self.header.size & PSEUDOVECTOR_SIZE_MASK) as EmacsInt
    }

    #[inline]
    pub fn as_bool_vector(&self) -> Option<LispBoolVecRef> {
        if self.is_pseudovector(PseudovecType::PVEC_BOOL_VECTOR) {
            Some(unsafe { mem::transmute::<_, LispBoolVecRef>(*self) })
        } else {
            None
        }
    }

    #[inline]
    pub fn as_buffer(&self) -> Option<LispBufferRef> {
        if self.is_pseudovector(PseudovecType::PVEC_BUFFER) {
            Some(unsafe { mem::transmute(*self) })
        } else {
            None
        }
    }

    #[inline]
    pub fn as_subr(&self) -> Option<LispSubrRef> {
        if self.is_pseudovector(PseudovecType::PVEC_SUBR) {
            Some(unsafe { mem::transmute(*self) })
        } else {
            None
        }
    }

    #[inline]
    pub fn as_window(&self) -> Option<LispWindowRef> {
        if self.is_pseudovector(PseudovecType::PVEC_WINDOW) {
            Some(unsafe { mem::transmute(*self) })
        } else {
            None
        }
    }

    #[inline]
    pub fn as_frame(&self) -> Option<LispFrameRef> {
        if self.is_pseudovector(PseudovecType::PVEC_FRAME) {
            Some(unsafe { mem::transmute(*self) })
        } else {
            None
        }
    }

    #[inline]
    pub fn as_process(&self) -> Option<LispProcessRef> {
        if self.is_pseudovector(PseudovecType::PVEC_PROCESS) {
            Some(unsafe { mem::transmute(*self) })
        } else {
            None
        }
    }

    #[inline]
    pub fn as_thread(&self) -> Option<ThreadStateRef> {
        if self.is_pseudovector(PseudovecType::PVEC_THREAD) {
            Some(unsafe { mem::transmute(*self) })
        } else {
            None
        }
    }

    #[inline]
    pub fn as_char_table(&self) -> Option<LispCharTableRef> {
        if self.is_pseudovector(PseudovecType::PVEC_CHAR_TABLE) {
            Some(unsafe { mem::transmute(*self) })
        } else {
            None
        }
    }

    pub fn as_sub_char_table(&self) -> Option<LispSubCharTableRef> {
        if self.is_pseudovector(PseudovecType::PVEC_SUB_CHAR_TABLE) {
            Some(unsafe { mem::transmute(*self) })
        } else {
            None
        }
    }

    pub fn as_sub_char_table_ascii(&self) -> Option<LispSubCharTableAsciiRef> {
        if self.is_pseudovector(PseudovecType::PVEC_SUB_CHAR_TABLE) {
            Some(unsafe { mem::transmute(*self) })
        } else {
            None
        }
    }

    #[inline]
    pub fn as_compiled(&self) -> Option<LispVectorlikeSlotsRef> {
        if self.is_pseudovector(PseudovecType::PVEC_COMPILED) {
            Some(unsafe { mem::transmute(*self) })
        } else {
            None
        }
    }
}

macro_rules! impl_vectorlike_ref {
    ($type:ident, $itertype:ident, $size_mask:expr) => {
        impl $type {
            #[inline]
            pub fn len(&self) -> usize {
                (self.header.size & $size_mask) as usize
            }

            #[inline]
            pub fn as_slice(&self) -> &[LispObject] {
                unsafe {
                    slice::from_raw_parts(
                        &self.contents as *const [::remacs_sys::Lisp_Object; 1]
                            as *const LispObject,
                        self.len(),
                    )
                }
            }

            #[inline]
            pub fn as_mut_slice(&mut self) -> &mut [LispObject] {
                unsafe {
                    slice::from_raw_parts_mut(
                        &mut self.contents as *mut [::remacs_sys::Lisp_Object; 1]
                            as *mut LispObject,
                        self.len(),
                    )
                }
            }

            #[inline]
            pub unsafe fn get_unchecked(&self, idx: ptrdiff_t) -> LispObject {
                ptr::read(
                    (&self.contents as *const [::remacs_sys::Lisp_Object; 1]
                     as *const LispObject).offset(idx),
                )
            }

            #[inline]
            pub unsafe fn set_unchecked(&mut self, idx: ptrdiff_t, item: LispObject) {
                ptr::write(
                    (&mut self.contents as *mut [::remacs_sys::Lisp_Object; 1]
                     as *mut LispObject).offset(idx),
                    item,
                )
            }

            #[inline]
            pub fn get(&self, idx: usize) -> LispObject {
                assert!(idx < self.len());
                unsafe { self.get_unchecked(idx as ptrdiff_t) }
            }

            #[inline]
            pub fn set(&mut self, idx: usize, item: LispObject) {
                assert!(idx < self.len());
                unsafe { self.set_unchecked(idx as ptrdiff_t, item) }
            }

            pub fn iter(&self) -> $itertype {
                $itertype::new(self)
            }
        }

        pub struct $itertype<'a> {
            vec: &'a $type,
            cur: usize,
            rev: usize,
        }

        impl<'a> $itertype<'a> {
            pub fn new(vec: &'a $type) -> Self {
                Self {
                    vec: vec,
                    cur: 0,
                    rev: vec.len(),
                }
            }
        }

        impl<'a> Iterator for $itertype<'a> {
            type Item = LispObject;

            fn next(&mut self) -> Option<Self::Item> {
                if self.cur < self.rev {
                    let res = unsafe { self.vec.get_unchecked(self.cur as ptrdiff_t) };
                    self.cur += 1;
                    Some(res)
                } else {
                    None
                }
            }
/*
            fn size_hint(&self) -> (usize, Option<usize>) {
                let remaining = (self.rev - self.cur) + 1;
                (remaining, Some(remaining))
            }
*/
        }

        impl<'a> DoubleEndedIterator for $itertype<'a> {
            fn next_back(&mut self) -> Option<Self::Item> {
                if self.rev > self.cur {
                    let res = unsafe { self.vec.get_unchecked((self.rev - 1) as ptrdiff_t) };
                    self.rev -= 1;
                    Some(res)
                } else {
                    None
                }
            }
        }

        impl<'a> ExactSizeIterator for $itertype<'a> {}
    }
}

impl_vectorlike_ref! { LispVectorRef, LispVecIterator, ptrdiff_t::max_value() }
impl_vectorlike_ref! { LispVectorlikeSlotsRef, LispVecSlotsIterator, PSEUDOVECTOR_SIZE_MASK }

impl LispBoolVecRef {
    #[inline]
    pub unsafe fn as_byte_ptr(&self) -> *const u8 {
        &self.data as *const [usize; 1] as *const u8
    }

    #[inline]
    pub unsafe fn as_mut_byte_ptr(&mut self) -> *mut u8 {
        &mut self.data as *mut [usize; 1] as *mut u8
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.size as usize
    }

    #[inline]
    unsafe fn get_bit(&self, idx: usize) -> bool {
        let limb = *self.as_byte_ptr().offset(idx as isize / 8);
        limb & (1 << (idx % 8)) != 0
    }

    #[inline]
    pub fn get(&self, idx: usize) -> LispObject {
        assert!(idx < self.len());
        unsafe { self.get_unchecked(idx) }
    }

    pub unsafe fn get_unchecked(&self, idx: usize) -> LispObject {
        LispObject::from_bool(self.get_bit(idx))
    }

    #[allow(dead_code)]
    #[inline]
    pub fn set_bit(&mut self, idx: usize, b: bool) {
        assert!(idx < self.len());
        let limbp = unsafe { self.as_mut_byte_ptr().offset(idx as isize / 8) };
        if b {
            unsafe { *limbp |= 1 << (idx % 8) }
        } else {
            unsafe { *limbp &= !(1 << (idx % 8)) }
        }
    }

    pub fn iter(&self) -> LispBoolVecIterator {
        LispBoolVecIterator {
            bvec: self,
            limb: 0,
            cur: 0,
        }
    }
}

pub struct LispBoolVecIterator<'a> {
    bvec: &'a LispBoolVecRef,
    limb: u8,
    cur: usize,
}

impl<'a> Iterator for LispBoolVecIterator<'a> {
    type Item = LispObject;

    fn next(&mut self) -> Option<LispObject> {
        if self.cur >= self.bvec.len() {
            None
        } else {
            if self.cur % 8 == 0 {
                self.limb = unsafe { *self.bvec.as_byte_ptr().offset(self.cur as isize / 8) };
            }
            let res = LispObject::from_bool(self.limb & (1 << (self.cur % 8)) != 0);
            self.cur += 1;
            Some(res)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.bvec.len() - self.cur;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for LispBoolVecIterator<'a> {}

/// Return the length of vector, list or string SEQUENCE.
/// A byte-code function object is also allowed.
/// If the string contains multibyte characters, this is not necessarily
/// the number of bytes in the string; it is the number of characters.
/// To get the number of bytes, use `string-bytes'.
#[lisp_fn]
pub fn length(sequence: LispObject) -> LispObject {
    if let Some(s) = sequence.as_string() {
        LispObject::from_natnum(s.len_chars() as EmacsInt)
    } else if let Some(vl) = sequence.as_vectorlike() {
        if let Some(v) = vl.as_vector() {
            LispObject::from_natnum(v.len() as EmacsInt)
        } else if let Some(bv) = vl.as_bool_vector() {
            LispObject::from_natnum(bv.len() as EmacsInt)
        } else if vl.is_pseudovector(PseudovecType::PVEC_CHAR_TABLE) {
            LispObject::from_natnum(EmacsInt::from(MAX_CHAR))
        } else if vl.is_pseudovector(PseudovecType::PVEC_COMPILED)
            || vl.is_pseudovector(PseudovecType::PVEC_RECORD)
        {
            LispObject::from_natnum(vl.pseudovector_size())
        } else {
            wrong_type!(Qsequencep, sequence);
        }
    } else if sequence.is_cons() {
        let len = sequence.iter_tails().count();
        if len > MOST_POSITIVE_FIXNUM as usize {
            error!("List too long");
        }
        LispObject::from_natnum(len as EmacsInt)
    } else if sequence.is_nil() {
        LispObject::from_natnum(0)
    } else {
        wrong_type!(Qsequencep, sequence);
    }
}

/// Return element of SEQUENCE at index N.
#[lisp_fn]
pub fn elt(sequence: LispObject, n: EmacsInt) -> LispObject {
    if sequence.is_cons() || sequence.is_nil() {
        nth(n, sequence)
    } else if sequence.is_array() {
        aref(sequence, n)
    } else {
        wrong_type!(Qsequencep, sequence);
    }
}

/// Sort SEQ, stably, comparing elements using PREDICATE.
/// Returns the sorted sequence.  SEQ should be a list or vector.  SEQ is
/// modified by side effects.  PREDICATE is called with two elements of
/// SEQ, and should return non-nil if the first element should sort before
/// the second.
#[lisp_fn]
pub fn sort(seq: LispObject, predicate: LispObject) -> LispObject {
    if seq.is_cons() {
        sort_list(seq, predicate)
    } else if let Some(mut vec) = seq.as_vectorlike().and_then(|v| v.as_vector()) {
        vec.as_mut_slice().sort_by(|&a, &b| {
            // XXX: since the `sort' predicate is a two-outcome comparison
            // Less/!Less, and slice::sort_by() uses Greater/!Greater
            // (which is not guaranteed anyway), this requires two calls
            // instead of one in some cases.
            if !inorder(predicate, a, b) {
                Ordering::Greater
            } else if !inorder(predicate, b, a) {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        });
        seq
    } else if seq.is_nil() {
        seq
    } else {
        wrong_type!(Qsequencep, seq)
    }
}

/// Return t if OBJECT is a vector.
#[lisp_fn]
pub fn vectorp(object: LispObject) -> bool {
    object.is_vector()
}

/// Return t if OBJECT is a char-table.
#[lisp_fn]
pub fn char_table_p(object: LispObject) -> bool {
    object.is_char_table()
}

/// Return t if OBJECT is a char-table or vector.
#[lisp_fn]
pub fn vector_or_char_table_p(object: LispObject) -> bool {
    object.is_vector() || object.is_char_table()
}

/// Return t if OBJECT is a bool-vector.
#[lisp_fn]
pub fn bool_vector_p(object: LispObject) -> bool {
    object.is_bool_vector()
}

/// Return t if OBJECT is an array (string or vector).
#[lisp_fn]
pub fn arrayp(object: LispObject) -> bool {
    object.is_array()
}

/// Return t if OBJECT is a sequence (list or array).
#[lisp_fn]
pub fn sequencep(object: LispObject) -> bool {
    object.is_sequence()
}

/// Return t if OBJECT is an editor buffer.
#[lisp_fn]
pub fn bufferp(object: LispObject) -> bool {
    object.is_buffer()
}

/// Return t if OBJECT is a built-in function.
#[lisp_fn]
pub fn subrp(object: LispObject) -> bool {
    object.is_subr()
}

/// Return t if OBJECT is a byte-compiled function object.
#[lisp_fn]
pub fn byte_code_function_p(object: LispObject) -> bool {
    object.is_byte_code_function()
}

/// Return t if OBJECT is a thread.
#[lisp_fn]
pub fn threadp(object: LispObject) -> bool {
    object.is_thread()
}

/// Return t if OBJECT is a mutex.
#[lisp_fn]
pub fn mutexp(object: LispObject) -> bool {
    object.is_mutex()
}

/// Return t if OBJECT is a condition variable.
#[lisp_fn]
pub fn condition_variable_p(object: LispObject) -> bool {
    object.is_condition_variable()
}

/// Return t if OBJECT is a record.
#[lisp_fn]
pub fn recordp(object: LispObject) -> bool {
    object.is_record()
}

lazy_static! {
    pub static ref HEADER_SIZE: usize = {
        unsafe { offset_of!(::remacs_sys::Lisp_Vector, contents) }
    };
    pub static ref WORD_SIZE: usize = {
        ::std::mem::size_of::<::remacs_sys::Lisp_Object>()
    };
}

include!(concat!(env!("OUT_DIR"), "/vectors_exports.rs"));
