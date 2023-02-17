use eyre::Result;
use url::Url;

pub fn base_url(mut url: Url) -> Result<Url> {
    {
        let mut path = url.path_segments_mut().unwrap();
        path.clear();
    }

    url.set_query(None);

    Ok(url)
}

pub fn extract_instance_url<S: AsRef<str>>(input: S) -> Result<Url> {
    base_url(Url::parse(input.as_ref())?)
}
