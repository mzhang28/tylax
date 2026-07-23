use typst::foundations::{Content, StyledElem, StyleChain, SequenceElem, Smart};
use typst::layout::PageElem;
use typst::layout::Sides;
use typst::layout::Rel;
use typst::layout::Length;

pub fn get_page_margin(content: &Content) -> Option<Sides<Option<Smart<Rel<Length>>>>> {
    if let Some(styled) = content.to_packed::<StyledElem>() {
        let chain = StyleChain::new(&styled.styles);
        let margin = chain.get(PageElem::margin);
        
        if let Smart::Custom(m) = margin {
            return Some(m.sides);
        }
    }
    
    if let Some(seq) = content.to_packed::<SequenceElem>() {
        for child in seq.children.iter() {
            if let Some(margin) = get_page_margin(child) {
                return Some(margin);
            }
        }
    }
    
    None
}

/// Extract the page `width`/`height` (in pt) as set by `#set page(...)`.
/// Returns `(width_pt, height_pt)`; either is `None` if never explicitly set
/// or set to `auto`.
///
/// NOTE: `PageElem::width`'s *default* is `Smart::Custom(A4)`, so a plain
/// `chain.get(...)` returns A4 for any style set that doesn't touch page width.
/// We therefore use `chain.has(...)` to only read the value from a style set
/// that actually assigns it, and take the last such assignment (later `#set`
/// rules win).
pub fn get_page_size(content: &Content) -> (Option<f64>, Option<f64>) {
    let mut width = None;
    let mut height = None;
    collect_page_size(content, &mut width, &mut height);
    (width, height)
}

fn collect_page_size(content: &Content, width: &mut Option<f64>, height: &mut Option<f64>) {
    if let Some(styled) = content.to_packed::<StyledElem>() {
        let chain = StyleChain::new(&styled.styles);
        if chain.has(PageElem::width) {
            if let Smart::Custom(len) = chain.get(PageElem::width) {
                *width = Some(len.abs.to_pt());
            }
        }
        if chain.has(PageElem::height) {
            if let Smart::Custom(len) = chain.get(PageElem::height) {
                *height = Some(len.abs.to_pt());
            }
        }
        collect_page_size(&styled.child, width, height);
    } else if let Some(seq) = content.to_packed::<SequenceElem>() {
        for child in seq.children.iter() {
            collect_page_size(child, width, height);
        }
    }
}

pub fn get_page_numbering(content: &Content) -> Option<String> {
    if let Some(styled) = content.to_packed::<StyledElem>() {
        let chain = StyleChain::new(&styled.styles);
        let numbering = chain.get_cloned(PageElem::numbering);
        if let Some(num) = numbering {
            return Some(format!("{:?}", num));
        }
    }
    
    if let Some(seq) = content.to_packed::<SequenceElem>() {
        for child in seq.children.iter() {
            if let Some(num) = get_page_numbering(child) {
                return Some(num);
            }
        }
    }
    
    None
}
