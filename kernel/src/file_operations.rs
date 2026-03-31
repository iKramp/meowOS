use std::{
    boxed::Box,
    mem_utils::{get_at_physical_addr, translate_phys_virt_addr},
    println, printlnc,
    string::String,
    vec::Vec,
};

use crate::{
    memory::physical_allocator,
    task_runner::block_task,
    vfs::{self, InodeType, file::FileFlags, open_file},
};

const BEE_MOVIE_SCRIPT_START: &str = include_str!("./bee_movie_script.txt");

const FILE_OPERATIONS: [FileOperation; 10] = [
    FileOperation::CreateFolder(CreateFolderOperation::new("/test")),
    FileOperation::ReadDir(ReadDirOperation::new("/")),
    FileOperation::CreateFile(CreateFileOperation::new("/test.txt")),
    FileOperation::CreateFile(CreateFileOperation::new("/test/test.txt")),
    FileOperation::ReadDir(ReadDirOperation::new("/")),
    FileOperation::Write(WriteFileOperation::new("/test.txt", "Hello, world!", 0)),
    FileOperation::Write(WriteFileOperation::new("/test/test.txt", BEE_MOVIE_SCRIPT_START, 0)),
    FileOperation::ReadFile(ReadFileOperation::new("/test.txt", 0, 5)),
    FileOperation::ReadFile(ReadFileOperation::new("/test.txt", 7, 6)),
    FileOperation::ReadFile(ReadFileOperation::new("/test/test.txt", 0, 49475)),
];

pub fn do_file_operations() {
    // memory::print_state();
    for opeartion in FILE_OPERATIONS {
        println!("Executing operation: {:?}", opeartion);
        opeartion.execute();
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
        let path = vfs::resolve_path(self.folder_name);
        let dir = block_task(Box::pin(open_file((&path).into(), None, FileFlags::new().with_read(true))))
            .expect("fopen failed in debug function");
        let entries = block_task(Box::pin(vfs::get_dir_entries(&dir))).expect("get dir entries failed in debug function");
        block_task(Box::pin(vfs::close_file(dir)));
        println!(level:info, "Read dir: {}", self.folder_name);
        println!(level:info, "Dir entries: {:?}", entries);
    }
}

impl CreateFileOperation {
    const fn new(file_name: &'static str) -> Self {
        Self { file_name }
    }

    fn execute(&self) {
        let split_index = self.file_name.rfind('/').expect("wawa");
        let (parent_path, file_name) = self.file_name.split_at(split_index + 1); //slash is in the
        println!(level:info, "Creating file: {}", self.file_name);
        let path = vfs::resolve_path(parent_path);

        let mut parent = block_task(Box::pin(open_file((&path).into(), None, FileFlags::new().with_write(true))))
            .expect("fopen failed in debug function");

        let _ = block_task(Box::pin(vfs::create_file(&mut parent, file_name, InodeType::new_file(0))));
        block_task(Box::pin(vfs::close_file(parent)));
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
            let frame = physical_allocator::allocate_frame();
            frames.push(frame);
            let frame_binding = translate_phys_virt_addr(frame);
            frame_bindings.push(frame_binding);
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
        let mut file =
            block_task(Box::pin(vfs::open_file((&path).into(), None, open_file_flags))).expect("fopen failed in debug function");

        println!(level:info, "Writing file: {} of size: {}", self.file_name, content.len());
        block_task(Box::pin(vfs::write_file(&mut file, &frames, self.content.len() as u64)))
            .expect("file write failed in debug function");
        block_task(Box::pin(vfs::close_file(file)));

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
        let split_index = self.folder_name.rfind('/').expect("wawa");
        let (parent_path, file_name) = self.folder_name.split_at(split_index + 1); //slash is in the
        println!(level:info, "Creating folder: {}", self.folder_name);
        let path = vfs::resolve_path(parent_path);

        let mut parent = block_task(Box::pin(open_file((&path).into(), None, FileFlags::new().with_write(true))))
            .expect("fopen failed in debug function");

        let _ = block_task(Box::pin(vfs::create_file(&mut parent, file_name, InodeType::new_dir(0))));
        block_task(Box::pin(vfs::close_file(parent)));
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
        let path = vfs::resolve_path(file_name);
        let real_offset = offset & !4095;
        let real_length = length - real_offset + offset;
        let mut buffer = Vec::with_capacity(real_length.div_ceil(4096) as usize);
        for _ in 0..(real_length.div_ceil(4096)) {
            let frame = crate::memory::physical_allocator::allocate_frame();
            buffer.push(frame);
        }

        let open_file_flags = FileFlags::new().with_read(true);
        let mut file =
            block_task(Box::pin(vfs::open_file((&path).into(), None, open_file_flags))).expect("fopen failed in debug function");

        block_task(Box::pin(vfs::read_file(&mut file, &buffer, real_length))).expect("file read failed in debug function");

        block_task(Box::pin(vfs::close_file(file)));

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

        println!(level:info, "Read file: {} at offset {} and size of read {}", file_name, offset, length);
        //transofm into string
        let string = String::from_utf8(final_data);
        match string {
            Ok(s) => {
                println!(level:info, "File content as string:");
                printlnc!(level:info, (255, 200, 100), "{}", s);
            }
            Err(_) => {
                println!(level:warn, "File content is not valid UTF-8, cannot display as string");
            }
        };
    }
}
