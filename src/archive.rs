extern crate zip;

use std::io::Read;
use std::io::Seek;
use std::fs::File;
use std::io::SeekFrom;
use std::collections::HashMap;
use std::ops::DerefMut;

use zip::read::ZipArchive;





pub trait Archive{

    fn list(&mut self )-> std::io::Result<Vec<String>>;

    fn entry<'a>(&'a mut self, path : &String)-> std::io::Result<Box<Read+'a>>;
}

struct WrappedZip<'a> (zip::read::ZipFile<'a>);

pub struct Zip <T: Seek + Read>{
    ar : Box<zip::ZipArchive<T> >
}



impl <T: Seek + Read> Archive for Zip<T>{

    fn list(&mut self )-> std::io::Result<Vec<String>>{

        let mut ret = Vec::new();

        for i in 0..self.ar.len(){
            let f = self.ar.by_index(i).unwrap();

            ret.push(String::from(f.name()));
        }

        return Ok(ret);
    }
    
    fn entry<'a>(&'a mut self, path : &String)->std::io::Result<Box<Read +'a>> {

        let temp = self.ar.by_name(path)?;

        let wrapped: Box<Read +'a> = Box::new(temp);

        return Ok(wrapped);
    }
}

impl <T :Read + Seek> Zip <T> {

    pub fn new (r : T)-> std::io::Result<Zip<T>>
    {


        let ar = zip::ZipArchive::new(r)?;

        let ret = Zip{
            ar:Box::new(ar)
        };

        Ok(ret)
    }
}

