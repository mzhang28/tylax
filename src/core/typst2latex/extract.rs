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
