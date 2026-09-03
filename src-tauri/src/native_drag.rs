







#![cfg(windows)]

use std::mem::ManuallyDrop;
use std::sync::Mutex;

use windows::core::*;
use windows::core::implement;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Memory::*;
use windows::Win32::System::Ole::*;
use windows::Win32::System::SystemServices::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;

#[implement(IDropSource, IDataObject)]
struct FileDragSource {
    files: Vec<(String, Vec<u8>)>,
}

fn cf_filegroupdescriptor() -> u16 {
    unsafe { RegisterClipboardFormatW(w!("FileGroupDescriptorW")) as u16 }
}

fn cf_filecontents() -> u16 {
    unsafe { RegisterClipboardFormatW(w!("FileContents")) as u16 }
}


fn clean_name(name: &str) -> String {
    let invalid: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    let mut s: String = name
        .chars()
        .map(|c| if invalid.contains(&c) { '_' } else { c })
        .collect();
    s = s.trim().trim_matches('.').to_string();
    if s.is_empty() {
        s = "image".to_string();
    }
    if s.len() > 200 {
        s = s[..200].to_string();
    }
    s
}

impl IDropSource_Impl for FileDragSource_Impl {
    fn QueryContinueDrag(&self, fescapepressed: BOOL, grfkeystate: MODIFIERKEYS_FLAGS) -> HRESULT {
        if fescapepressed.as_bool() {
            return DRAGDROP_S_CANCEL;
        }
        
        if (grfkeystate.0 & MK_LBUTTON.0) == 0 {
            return DRAGDROP_S_DROP;
        }
        S_OK
    }

    fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> HRESULT {
        
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

impl IDataObject_Impl for FileDragSource_Impl {
    fn GetData(&self, pformatetc: *const FORMATETC) -> Result<STGMEDIUM> {
        unsafe {
            if pformatetc.is_null() {
                return Err(Error::new(E_INVALIDARG, ""));
            }
            let etc = *pformatetc;
            let cf_desc = cf_filegroupdescriptor();
            let cf_cont = cf_filecontents();
            
            if etc.cfFormat == cf_desc && (etc.tymed & TYMED_HGLOBAL.0 as u32) != 0 {
                let names: Vec<String> = self.files.iter().map(|(n, _)| n.clone()).collect();
                let sizes: Vec<usize> = self.files.iter().map(|(_, b)| b.len()).collect();
                return Ok(build_filegroup_multi(&names, &sizes));
            }
            
            if etc.cfFormat == cf_cont && (etc.tymed & TYMED_HGLOBAL.0 as u32) != 0 {
                let mut idx = etc.lindex;
                if idx < 0 {
                    idx = 0;
                }
                if idx as usize >= self.files.len() {
                    return Err(Error::new(DV_E_FORMATETC, ""));
                }
                return Ok(build_filecontents(&self.files[idx as usize].1));
            }
            Err(Error::new(DV_E_FORMATETC, ""))
        }
    }

    fn GetDataHere(&self, _pformatetc: *const FORMATETC, _pmedium: *mut STGMEDIUM) -> Result<()> {
        Err(Error::new(E_NOTIMPL, ""))
    }

    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        unsafe {
            if pformatetc.is_null() {
                return E_INVALIDARG;
            }
            let etc = *pformatetc;
            let cf_desc = cf_filegroupdescriptor();
            let cf_cont = cf_filecontents();
            if (etc.cfFormat == cf_desc || etc.cfFormat == cf_cont)
                && (etc.tymed & TYMED_HGLOBAL.0 as u32) != 0
            {
                
                if etc.cfFormat == cf_cont
                    && etc.lindex >= 0
                    && (etc.lindex as usize) >= self.files.len()
                {
                    return DV_E_FORMATETC;
                }
                S_OK
            } else {
                DV_E_FORMATETC
            }
        }
    }

    fn GetCanonicalFormatEtc(
        &self,
        _pformatectin: *const FORMATETC,
        pformatetcout: *mut FORMATETC,
    ) -> HRESULT {
        unsafe {
            if !pformatetcout.is_null() {
                *pformatetcout = std::mem::zeroed();
            }
            E_NOTIMPL
        }
    }

    fn SetData(
        &self,
        _pformatetc: *const FORMATETC,
        _pmedium: *const STGMEDIUM,
        _frelease: BOOL,
    ) -> Result<()> {
        Err(Error::new(E_NOTIMPL, ""))
    }

    fn EnumFormatEtc(&self, _dwdirection: u32) -> Result<IEnumFORMATETC> {
        let cf_desc = cf_filegroupdescriptor();
        let cf_cont = cf_filecontents();
        let mk = |cf: u16| FORMATETC {
            cfFormat: cf,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0 as u32,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        let fmts = vec![mk(cf_desc), mk(cf_cont)];
        Ok(FileEnumFormatEtc {
            index: Mutex::new(0),
            fmts,
        }
        .into())
    }

    fn DAdvise(
        &self,
        _pformatetc: *const FORMATETC,
        _advf: u32,
        _padvsink: Ref<'_, IAdviseSink>,
    ) -> Result<u32> {
        Err(Error::new(OLE_E_ADVISENOTSUPPORTED, ""))
    }

    fn DUnadvise(&self, _dwconnection: u32) -> Result<()> {
        Err(Error::new(E_NOTIMPL, ""))
    }

    fn EnumDAdvise(&self) -> Result<IEnumSTATDATA> {
        Err(Error::new(E_NOTIMPL, ""))
    }
}




fn build_filegroup_multi(names: &[String], sizes: &[usize]) -> STGMEDIUM {
    unsafe {
        let n = names.len();
        let total = std::mem::size_of::<FILEGROUPDESCRIPTORW>()
            + (n - 1) * std::mem::size_of::<FILEDESCRIPTORW>();
        let hglobal = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, total)
            .expect("alloc FileGroupDescriptor");
        let ptr = GlobalLock(hglobal) as *mut FILEGROUPDESCRIPTORW;
        (*ptr).cItems = n as u32;
        
        
        let fgd_base = (ptr as *mut u8)
            .add(std::mem::size_of::<u32>())
            as *mut FILEDESCRIPTORW;
        for i in 0..n {
            let mut fname = [0u16; 260];
            let nw: Vec<u16> = clean_name(&names[i]).encode_utf16().collect();
            for (j, w) in nw.iter().take(259).enumerate() {
                fname[j] = *w;
            }
            let fgd = FILEDESCRIPTORW {
                dwFlags: FD_FILESIZE.0 as u32,
                clsid: GUID {
                    data1: 0,
                    data2: 0,
                    data3: 0,
                    data4: [0; 8],
                },
                sizel: SIZE { cx: 0, cy: 0 },
                pointl: POINTL { x: 0, y: 0 },
                nFileSizeLow: (sizes[i] & 0xFFFF_FFFF) as u32,
                nFileSizeHigh: (sizes[i] >> 32) as u32,
                ftCreationTime: FILETIME {
                    dwLowDateTime: 0,
                    dwHighDateTime: 0,
                },
                ftLastAccessTime: FILETIME {
                    dwLowDateTime: 0,
                    dwHighDateTime: 0,
                },
                ftLastWriteTime: FILETIME {
                    dwLowDateTime: 0,
                    dwHighDateTime: 0,
                },
                dwFileAttributes: 0,
                cFileName: fname,
            };
            if i == 0 {
                (*ptr).fgd[0] = fgd;
            } else {
                *fgd_base.add(i) = fgd;
            }
        }
        GlobalUnlock(hglobal).ok();
        STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: hglobal },
            pUnkForRelease: ManuallyDrop::new(None),
        }
    }
}


fn build_filecontents(bytes: &[u8]) -> STGMEDIUM {
    unsafe {
        let total = bytes.len().max(1);
        let hglobal = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, total).expect("alloc FileContents");
        let ptr = GlobalLock(hglobal) as *mut u8;
        if !bytes.is_empty() {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        }
        GlobalUnlock(hglobal).ok();
        STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: hglobal },
            pUnkForRelease: ManuallyDrop::new(None),
        }
    }
}

#[implement(IEnumFORMATETC)]
struct FileEnumFormatEtc {
    index: Mutex<u32>,
    fmts: Vec<FORMATETC>,
}

impl IEnumFORMATETC_Impl for FileEnumFormatEtc_Impl {
    fn Next(&self, celt: u32, rgelt: *mut FORMATETC, pceltfetched: *mut u32) -> HRESULT {
        unsafe {
            let mut fetched: u32 = 0;
            let mut i = self.index.lock().unwrap();
            while *i < self.fmts.len() as u32 && fetched < celt {
                *rgelt.add(fetched as usize) = self.fmts[*i as usize];
                *i += 1;
                fetched += 1;
            }
            if !pceltfetched.is_null() {
                *pceltfetched = fetched;
            }
            if fetched == celt {
                S_OK
            } else {
                S_FALSE
            }
        }
    }

    fn Skip(&self, celt: u32) -> Result<()> {
        *self.index.lock().unwrap() += celt;
        Ok(())
    }

    fn Reset(&self) -> Result<()> {
        *self.index.lock().unwrap() = 0;
        Ok(())
    }

    fn Clone(&self) -> Result<IEnumFORMATETC> {
        Ok(FileEnumFormatEtc {
            index: Mutex::new(*self.index.lock().unwrap()),
            fmts: self.fmts.clone(),
        }
        .into())
    }
}



pub fn do_file_drag(files: Vec<(String, Vec<u8>)>) -> Result<()> {
    unsafe {
        let total_bytes: usize = files.iter().map(|(_, b)| b.len()).sum();
        crate::drag_log(&format!(
            "do_file_drag start files={} total_bytes={}",
            files.len(),
            total_bytes
        ));
        let data_obj = match build_data_object(files) {
            Ok(o) => o,
            Err(e) => {
                crate::drag_log(&format!("do_file_drag build_data_object ERR: {}", e));
                return Err(e);
            }
        };
        let drop_source: IDropSource = match data_obj.cast() {
            Ok(d) => d,
            Err(e) => {
                crate::drag_log(&format!("do_file_drag cast IDropSource ERR: {}", e));
                return Err(e);
            }
        };
        let mut effect = DROPEFFECT_COPY;
        
        
        let _ = DoDragDrop(&data_obj, &drop_source, DROPEFFECT_COPY, &mut effect);
        crate::drag_log(&format!(
            "do_file_drag done: effect={} (1=COPY 已落盘, 0=取消/无目标)",
            effect.0
        ));
        Ok(())
    }
}



pub fn build_data_object(files: Vec<(String, Vec<u8>)>) -> Result<IDataObject> {
    let source: IDropSource = FileDragSource { files }.into();
    Ok(source.cast()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Com::CoInitializeEx;
    use windows::Win32::System::Com::COINIT_APARTMENTTHREADED;

    fn mk_fmt(cf: u16) -> FORMATETC {
        FORMATETC {
            cfFormat: cf,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0 as u32,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        }
    }

    #[test]
    fn single_file_produces_valid_descriptor_and_contents() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let bytes: &[u8] = b"\x89PNG\r\n\x1a\n-fake";
            let data_obj = build_data_object(vec![("pic.png".to_string(), bytes.to_vec())]).expect("build");

            let cf_desc = cf_filegroupdescriptor();
            let cf_cont = cf_filecontents();

            let medium = data_obj.GetData(&mk_fmt(cf_desc)).expect("GetData descriptor");
            assert_eq!(medium.tymed, TYMED_HGLOBAL.0 as u32, "tymed must be HGLOBAL");
            let hglobal = medium.u.hGlobal;
            assert!(!hglobal.is_invalid(), "hGlobal must be valid");

            let ptr = GlobalLock(hglobal) as *const FILEGROUPDESCRIPTORW;
            let c_items: u32 = std::ptr::addr_of!((*ptr).cItems).read_unaligned();
            assert_eq!(c_items, 1, "must describe exactly 1 file");
            let fname_arr: [u16; 260] = std::ptr::addr_of!((*ptr).fgd[0].cFileName).read_unaligned();
            let s: String = fname_arr
                .iter()
                .take_while(|&&c| c != 0)
                .map(|&c| char::from_u32(c as u32).unwrap_or('?'))
                .collect();
            GlobalUnlock(hglobal).ok();
            assert_eq!(s, "pic.png", "filename in descriptor must match");

            let m2 = data_obj.GetData(&mk_fmt(cf_cont)).expect("GetData contents");
            let h2 = m2.u.hGlobal;
            let cp = GlobalLock(h2) as *const u8;
            let got = std::slice::from_raw_parts(cp, bytes.len());
            GlobalUnlock(h2).ok();
            assert_eq!(got, bytes, "FileContents bytes must equal source");
        }
    }

    #[test]
    fn multi_file_returns_each_content_by_index() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let bytes_a: &[u8] = b"\x89PNG-aaaa";
            let bytes_b: &[u8] = b"\xff\xd8-jpgb";
            let data_obj = build_data_object(vec![
                ("a.png".to_string(), bytes_a.to_vec()),
                ("b.jpg".to_string(), bytes_b.to_vec()),
            ])
            .expect("build");

            let cf_desc = cf_filegroupdescriptor();
            let cf_cont = cf_filecontents();

            
            let medium = data_obj.GetData(&mk_fmt(cf_desc)).expect("GetData descriptor");
            let ptr = GlobalLock(medium.u.hGlobal) as *const FILEGROUPDESCRIPTORW;
            let c_items: u32 = std::ptr::addr_of!((*ptr).cItems).read_unaligned();
            assert_eq!(c_items, 2, "must describe exactly 2 files");
            GlobalUnlock(medium.u.hGlobal).ok();

            
            let mut f0 = mk_fmt(cf_cont);
            f0.lindex = 0;
            let m0 = data_obj.GetData(&f0).expect("GetData content[0]");
            let p0 = GlobalLock(m0.u.hGlobal) as *const u8;
            let g0 = std::slice::from_raw_parts(p0, bytes_a.len());
            GlobalUnlock(m0.u.hGlobal).ok();
            assert_eq!(g0, bytes_a, "content[0] must be file a");

            let mut f1 = mk_fmt(cf_cont);
            f1.lindex = 1;
            let m1 = data_obj.GetData(&f1).expect("GetData content[1]");
            let p1 = GlobalLock(m1.u.hGlobal) as *const u8;
            let g1 = std::slice::from_raw_parts(p1, bytes_b.len());
            GlobalUnlock(m1.u.hGlobal).ok();
            assert_eq!(g1, bytes_b, "content[1] must be file b");

            
            let mut f2 = mk_fmt(cf_cont);
            f2.lindex = 2;
            assert!(data_obj.GetData(&f2).is_err(), "out-of-range index must fail");
        }
    }
}
