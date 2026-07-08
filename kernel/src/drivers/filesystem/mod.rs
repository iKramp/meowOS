pub mod fat;
pub mod rfs2;
//rfs stays in the codebase for reference, but not used

pub(super) fn init_fs_drivers() {
    rfs2::init_rfs2();
    fat::init_fat();
}
