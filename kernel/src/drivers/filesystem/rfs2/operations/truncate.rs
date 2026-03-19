use crate::drivers::filesystem::rfs2::Rfs2;

impl Rfs2 {
    pub(super) async fn truncate_locked(&self, file_root: u64, new_size_bytes: usize) {
        todo!();
    }
}
