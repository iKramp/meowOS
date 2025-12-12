use std::{boxed::Box, mem_utils::get_at_physical_addr, println, printlnc, string::String, vec::Vec};

use crate::{
    memory::PAGE_TREE_ALLOCATOR,
    task_runner::{add_task, block_task},
    vfs::{self, file::FileFlags},
};

const BEE_MOVIE_SCRIPT_START: &str = include_str!("./bee_movie_script.txt");

const FILE_OPERATIONS: [FileOperation; 1] = [
    // FileOperation::CreateFolder(CreateFolderOperation::new("/test")),
    // FileOperation::ReadDir(ReadDirOperation::new("/")),
    // FileOperation::CreateFile(CreateFileOperation::new("/test.txt")),
    // FileOperation::CreateFile(CreateFileOperation::new("/test/test.txt")),
    // FileOperation::ReadDir(ReadDirOperation::new("/")),
    // FileOperation::Write(WriteFileOperation::new("/test.txt", "Hello, world!", 0)),
    // FileOperation::Write(WriteFileOperation::new("/test/test.txt", BEE_MOVIE_SCRIPT_START, 0)),
    // FileOperation::ReadFile(ReadFileOperation::new("/test.txt", 0, 5)),
    // FileOperation::ReadFile(ReadFileOperation::new("/test.txt", 7, 6)),
    FileOperation::ReadFile(ReadFileOperation::new("/test/test.txt", 0, 49475)),
];

pub fn do_file_operations() {
    // memory::print_state();
    for opeartion in FILE_OPERATIONS {
        // println!("Executing operation: {:?}", opeartion);
        opeartion.execute();
        // println!("Memory state after execution: {:?}", opeartion);
        // memory::print_state();
    }
}

#[derive(Debug)]
enum FileOperation {
    CreateFile(CreateFileOperation),
    ReadDir(ReadDirOperation),
    Write(WriteFileOperation),
    Delete(DeleteFileOperation),
    CreateFolder(CreateFolderOperation),
    ReadFile(ReadFileOperation),
}

impl FileOperation {
    fn execute(&self) {
        match self {
            Self::ReadFile(op) => op.execute(),
            Self::CreateFile(op) => op.execute(),
            Self::Write(op) => op.execute(),
            Self::Delete(op) => op.execute(),
            Self::CreateFolder(op) => op.execute(),
            Self::ReadDir(op) => op.execute(),
        }
    }
}

#[derive(Debug)]
struct CreateFileOperation {
    file_name: &'static str,
}

#[derive(Debug)]
struct ReadDirOperation {
    folder_name: &'static str,
}

impl ReadDirOperation {
    const fn new(folder_name: &'static str) -> Self {
        Self { folder_name }
    }

    fn execute(&self) {
        // let path = vfs::resolve_path(self.folder_name);
        // let entries = block_task(Box::pin(vfs::get_dir_entries((&path).into())));
        // println!("Read dir: {}", self.folder_name);
        // println!("Dir entries: {:?}", entries);
    }
}

impl CreateFileOperation {
    const fn new(file_name: &'static str) -> Self {
        Self { file_name }
    }

    fn execute(&self) {
        // let split_index = self.file_name.rfind('/').unwrap();
        // let (parent_path, file_name) = self.file_name.split_at(split_index + 1); //slash is in the
        // println!("Creating file: {}", self.file_name);
        // let path = vfs::resolve_path(parent_path);
        // block_task(Box::pin(vfs::create_file((&path).into(), file_name, InodeType::new_file(0))));
        // println!("Created file: {:?}", self.file_name);
    }
}

#[derive(Debug)]
struct WriteFileOperation {
    file_name: &'static str,
    content: &'static str,
    offset: u64,
}

impl WriteFileOperation {
    const fn new(file_name: &'static str, content: &'static str, offset: u64) -> Self {
        Self {
            file_name,
            content,
            offset,
        }
    }

    fn execute(&self) {
        let path = vfs::resolve_path(self.file_name);

        let content = self.content.as_bytes();
        let mut frames = Vec::new();
        let mut frame_bindings = Vec::new();
        for _ in 0..(content.len().div_ceil(4096)) {
            let frame = crate::memory::physical_allocator::allocate_frame();
            frames.push(frame);
            let frame_binding = unsafe { PAGE_TREE_ALLOCATOR.allocate(Some(frame), false) };
            frame_bindings.push(frame_binding);
            unsafe {
                PAGE_TREE_ALLOCATOR
                    .get_page_table_entry_mut(frame_binding)
                    .expect("page entry must exist after allocation")
                    .set_pat(crate::memory::paging::LiminePat::UC);
            }
        }
        for i in 0..(content.len().div_ceil(4096)) {
            let ptr = frame_bindings[i].0 as *mut u8;
            for j in 0..4096 {
                if (i * 4096 + j) < content.len() {
                    unsafe { *ptr.add(j) = content[i * 4096 + j] };
                } else {
                    unsafe { *ptr.add(j) = 0 };
                }
            }
        }

        let open_file_flags = FileFlags::new().with_write(true);
        let mut file = block_task(Box::pin(vfs::open_file((&path).into(), None, open_file_flags))).expect("fopen failed in debug function");

        println!("Writing file: {} of size: {}", self.file_name, content.len());
        block_task(Box::pin(vfs::write_file(
            &mut file,
            &frames,
            self.content.len() as u64,
        )))
        .expect("file write failed in debug function");

        for frame in frames {
            unsafe { crate::memory::physical_allocator::deallocate_frame(frame) };
        }
    }
}

#[derive(Debug)]
struct CreateFolderOperation {
    folder_name: &'static str,
}

impl CreateFolderOperation {
    const fn new(folder_name: &'static str) -> Self {
        Self { folder_name }
    }

    fn execute(&self) {
        todo!();
        // let split_index = self.folder_name.rfind('/').unwrap();
        // let (parent_path, file_name) = self.folder_name.split_at(split_index + 1); //slash is in the
        // println!("Creating folder: {}", self.folder_name);
        // let path = vfs::resolve_path(parent_path, "/");
        // block_task(Box::pin(vfs::create_file((&path).into(), file_name, InodeType::new_file(0))));
    }
}

#[derive(Debug)]
struct DeleteFileOperation {
    file_name: &'static str,
}

impl DeleteFileOperation {
    const fn new(file_name: &'static str) -> Self {
        Self { file_name }
    }

    fn execute(&self) {
        todo!("add vfs delete op");
    }
}

#[derive(Debug)]
struct ReadFileOperation {
    file_name: &'static str,
    offset: u64,
    length: u64,
}

impl ReadFileOperation {
    const fn new(file_name: &'static str, offset: u64, length: u64) -> Self {
        Self {
            file_name,
            offset,
            length,
        }
    }

    fn execute(&self) {
        let offset = self.offset;
        let length = self.length;
        let file_name = self.file_name;
        let task = async move {
            let path = vfs::resolve_path(file_name);
            let real_offset = offset & !4095;
            let real_length = length - real_offset + offset;
            let mut buffer = Vec::with_capacity(real_length.div_ceil(4096) as usize);
            for _ in 0..(real_length.div_ceil(4096)) {
                let frame = crate::memory::physical_allocator::allocate_frame();
                buffer.push(frame);
            }

            let open_file_flags = FileFlags::new().with_read(true);
            let mut file = vfs::open_file((&path).into(), None, open_file_flags).await.expect("fopen failed in debug function");

            vfs::read_file(&mut file, &buffer, real_length).await.expect("file read failed in debug function");
            let mut final_data = Vec::with_capacity(length as usize);
            let mut frame_ptr = (offset as usize) & 0xFFF;
            for (index, frame) in buffer.iter().enumerate() {
                let frame_ptr_start = frame_ptr;
                let limit = if index == buffer.len() - 1 {
                    (length as usize) & 0xFFF
                } else {
                    4096
                };
                let data = unsafe { get_at_physical_addr::<[u8; 4096]>(*frame) };
                while frame_ptr < limit + frame_ptr_start {
                    final_data.push(data[frame_ptr & 0xFFF]);
                    frame_ptr += 1;
                }
                unsafe { crate::memory::physical_allocator::deallocate_frame(*frame) };
            }

            println!(
                "Read file: {} at offset {} and size of read {}",
                file_name, offset, length
            );
            // println!("File content: {:?}", final_data);
            //transofm into string
            let string = String::from_utf8(final_data).unwrap_or(String::from(""));
            println!("File content as string:");
            printlnc!((255, 200, 100), "{}", string);
        };
        add_task(Box::pin(task), None);
    }
}
