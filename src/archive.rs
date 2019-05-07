extern crate zip;

use std::io::Read;
use std::io::Seek;
use std::fs::File;
use std::io::SeekFrom;
use std::collections::HashMap;
use std::ops::DerefMut;

use zip::read::ZipArchive;



pub trait ReadAndSeek : Read + Seek{
}

impl ReadAndSeek for std::fs::File{
}

pub trait Archive <'a>{

    fn list(&mut self )-> std::io::Result<Vec<String>>;

    fn entry(&'a mut self, path : &String)-> std::io::Result<&'a ReadAndSeek>;
}

struct WrappedZip<'a> (zip::read::ZipFile<'a>);

pub struct Zip <'a,T: ReadAndSeek>{

    ar : Box<zip::ZipArchive<T> >,
    str_file_map: HashMap<String,WrappedZip<'a> >
}

impl <'a> Seek for WrappedZip<'a>{
 fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64>{
     Ok(0)
 }
}

impl <'a> Read for WrappedZip<'a>{

    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>{
        Ok(0)
    }
}


impl <'a>ReadAndSeek for WrappedZip<'a>{

}


impl <'a,T:ReadAndSeek > Archive<'a> for Zip<'a,T>{

    fn list(&mut self )-> std::io::Result<Vec<String>>{

        let mut ret = Vec::new();

        for i in 0..self.ar.len(){
            let f = self.ar.by_index(i).unwrap();

            ret.push(String::from(f.name()));
        }

        return Ok(ret);
    }
    
    fn entry(&'a mut self, path : &String)->std::io::Result<&'a ReadAndSeek>{

        let temp : zip::read::ZipFile<'a> = self.ar.by_name(path)?;

        self.str_file_map.insert(path.clone(),WrappedZip(temp));

        let ret : &WrappedZip<'a> = self.str_file_map.get(path).unwrap();

        return Ok(ret);
    }
}

impl <'a,T :ReadAndSeek> Zip <'a,T> {

    pub fn new (r : T)-> std::io::Result<Zip<'a,T>>
    {


        let ar = zip::ZipArchive::new(r)?;

        let ret = Zip{
            ar:Box::new(ar),
            str_file_map: HashMap::new()
        };

        Ok(ret)
    }
}

