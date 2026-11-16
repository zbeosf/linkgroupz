use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// 임시 파일에 쓴 뒤 이름을 바꿔 넣는다. 쓰는 도중 꺼져도 원래 파일이 반쯤 쓰인 채 남지 않는다.
pub fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let file_name = path.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, crate::tr!("파일 이름이 없습니다", "The path has no file name")))?;
    let mut temp_name = std::ffi::OsString::from(".");
    temp_name.push(file_name);
    temp_name.push(format!(".{}-{}.tmp", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
    let temp = path.with_file_name(temp_name);

    let result = (|| {
        let mut file = fs::File::create(&temp)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// 폴더를 통째로 복사한다 (대상 폴더는 새로 만든다).
pub fn copy_dir(source: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let destination = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &destination)?;
        } else {
            fs::copy(entry.path(), &destination)?;
        }
    }
    Ok(())
}
