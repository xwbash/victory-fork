//! Query heaps. Occlusion heaps can create a host `ID3D12QueryHeap` when
//! `VICTORY_REAL_OCCLUSION=1` is set; otherwise queries are answered with
//! synthetic values on resolve.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use crate::object::{self, E_INVALIDARG, HRESULT, Iid, Object, QUERY_HEAP, Table, This, guid};
use crate::resource::PAGEABLE_IIDS;

#[cfg(windows)]
use windows::Win32::Graphics::Direct3D12::{
    D3D12_QUERY_HEAP_DESC, D3D12_QUERY_HEAP_TYPE_OCCLUSION, D3D12_QUERY_TYPE,
    D3D12_QUERY_TYPE_BINARY_OCCLUSION, D3D12_QUERY_TYPE_OCCLUSION, ID3D12QueryHeap,
};

const HEAP_IIDS: [Iid; 4] = [
    guid(
        0x0d9658ae,
        0xed45,
        0x469e,
        [0xa6, 0x1d, 0x97, 0x0e, 0xc5, 0x83, 0xca, 0xb4],
    ),
    PAGEABLE_IIDS[0],
    PAGEABLE_IIDS[1],
    PAGEABLE_IIDS[2],
];

/// Whether real host occlusion queries (`BeginQuery`/`EndQuery`/`ResolveQueryData`)
/// are enabled (`VICTORY_REAL_OCCLUSION=1`; off by default).
pub(crate) fn real_occlusion() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("VICTORY_REAL_OCCLUSION").is_some_and(|v| v == "1"))
}

/// Maps a console query kind to a host D3D12 occlusion query type.
#[cfg(windows)]
pub(crate) fn occlusion_query_type(kind: u32) -> Option<D3D12_QUERY_TYPE> {
    match kind {
        0 => Some(D3D12_QUERY_TYPE_OCCLUSION),
        1 => Some(D3D12_QUERY_TYPE_BINARY_OCCLUSION),
        _ => None,
    }
}

/// Validates that `[first, first + count)` fits inside a heap of `heap_count` queries.
pub(crate) fn resolve_range_valid(first: u32, count: u32, heap_count: u32) -> bool {
    count > 0 && first.checked_add(count).is_some_and(|end| end <= heap_count)
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct Desc {
    kind: u32,
    count: u32,
    nodes: u32,
}

#[repr(C)]
struct Heap {
    object: Object,
    desc: Desc,
    #[cfg(windows)]
    host: Option<ID3D12QueryHeap>,
}

/// Returns the host occlusion query heap and its query count, if created.
#[cfg(windows)]
pub(crate) fn host_heap(this: This) -> Option<(ID3D12QueryHeap, u32)> {
    if this.is_null() {
        return None;
    }
    // SAFETY: the title passes a live query heap created by `create`.
    let heap = unsafe { &*this.cast::<Heap>() };
    Some((heap.host.as_ref()?.clone(), heap.desc.count))
}

#[cfg(windows)]
fn make_host_heap(desc: Desc) -> Option<ID3D12QueryHeap> {
    if !real_occlusion() || desc.kind != 0 || desc.count == 0 {
        return None;
    }
    let host = crate::host::host()?;
    let d3d_desc = D3D12_QUERY_HEAP_DESC {
        Type: D3D12_QUERY_HEAP_TYPE_OCCLUSION,
        Count: desc.count,
        NodeMask: 0,
    };
    let mut out: Option<ID3D12QueryHeap> = None;
    // SAFETY: valid description on a live host device.
    match unsafe { host.device.CreateQueryHeap(&d3d_desc, &mut out) } {
        Ok(()) => out,
        Err(e) => {
            tracing::warn!(error = %e, count = desc.count, "host occlusion query heap not created");
            None
        }
    }
}

/// Device method.
pub(crate) extern "system" fn create(
    _: This,
    desc: *const Desc,
    iid: *const Iid,
    out: *mut This,
) -> HRESULT {
    // SAFETY: the title passes a description (null is tolerated).
    let Some(&desc) = (unsafe { desc.as_ref() }) else {
        return E_INVALIDARG;
    };
    tracing::debug!(?desc, "query heap");
    let heap = Box::new(Heap {
        object: Object::new(&FUNCTIONS),
        desc,
        #[cfg(windows)]
        host: make_host_heap(desc),
    });
    object::hand_out(QUERY_HEAP, heap, iid, &HEAP_IIDS, out)
}

extern "system" fn query(this: This, iid: *const Iid, out: *mut This) -> HRESULT {
    object::answer(
        QUERY_HEAP,
        iid,
        object::is(iid, &HEAP_IIDS).then_some(this),
        out,
    )
}
extern "system" fn release(this: This) -> u32 {
    object::release_boxed::<Heap>(this)
}

/// IUnknown 3, object 4, device child 1.
static FUNCTIONS: Table<24> = {
    let mut f = object::d3d12_object(object::missing_table::<QUERY_HEAP, 24>());
    f[0] = query as *const ();
    f[2] = release as *const ();
    f[7] = object::parent_device as *const (); // 0x38
    Table(f)
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_range_valid_rejects_empty_and_overflow() {
        assert!(resolve_range_valid(0, 1, 64));
        assert!(resolve_range_valid(63, 1, 64));
        assert!(!resolve_range_valid(0, 0, 64));
        assert!(!resolve_range_valid(64, 1, 64));
        assert!(!resolve_range_valid(u32::MAX, 1, u32::MAX));
    }

    #[cfg(windows)]
    #[test]
    fn occlusion_query_type_maps_only_occlusion_kinds() {
        assert_eq!(occlusion_query_type(0), Some(D3D12_QUERY_TYPE_OCCLUSION));
        assert_eq!(occlusion_query_type(1), Some(D3D12_QUERY_TYPE_BINARY_OCCLUSION));
        assert_eq!(occlusion_query_type(2), None);
        assert_eq!(occlusion_query_type(3), None);
    }
}
