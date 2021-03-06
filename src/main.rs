use console::{Key, Term};
use reqwest::blocking;
use scraper::Selector;
use std::io::Write;

mod story;
mod tags;

use story::{display_story, Story};
use tags::Tag;

const URL: &str = "https://lobste.rs";
const PAGE_URL: &str = "https://lobste.rs/page";
const STORIES_PER_SITE_PAGE: usize = 25;

// fn stories_window(stories: Vec<Story>, start: usize, n: usize) -> Vec<Story> {
//     let end = usize::min(start + n, stories.len());
//     stories[start..end].to_vec()
// }

fn get_stories(page: u16) -> Result<Vec<Story>, Box<dyn std::error::Error>> {
    let url = format!("{PAGE_URL}/{page}");
    let client = blocking::Client::new();
    #[cfg(debug_assertions)]
    eprint!("loading page {page} ({url}) ...");
    #[cfg(not(debug_assertions))]
    eprint!("loading page {page} ...");
    let res = client
        .get(url)
        .header(
            "user-agent",
            "Mozilla/5.0 (X11; Linux x86_64; rv:12.0) Gecko/20100101 Firefox/12.0",
        )
        .send()?;
    let status = res.error_for_status_ref()?.status();
    #[cfg(debug_assertions)]
    eprintln!("\u{1b}[2K\rloaded ({status})");

    let html = scraper::Html::parse_document(&res.text()?);
    let stories_selector = Selector::parse("ol.stories > .story > .story_liner").unwrap();
    let stories_list = html.select(&stories_selector);
    Ok(stories_list.map(Story::from_html).collect())
}

struct View {
    stories: Vec<Story>,
    pos: usize,
    page_size: usize,
}

impl View {
    /// Creates a new empty view.
    fn new(page_size: usize) -> Self {
        Self {
            stories: Vec::new(),
            pos: 0,
            page_size,
        }
    }

    /// Load the next page of stories according to the number of stories stored.
    fn load_stories(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // The page numbers of the site are 1-indexed.
        let stories = &mut get_stories((self.stories.len() / STORIES_PER_SITE_PAGE) as u16 + 1)?;
        self.stories.append(stories);
        Ok(())
    }

    /// Get new pages of stories until the number of stories stored exceeds `pos`.
    fn load_stories_including(&mut self, pos: usize) -> Result<(), Box<dyn std::error::Error>> {
        while self.stories.len() <= pos {
            self.load_stories()?;
        }

        Ok(())
    }

    /// Get a new page of stories when pos steps onto the next presentation page and the current
    /// list of stories does not extend far enough to fill that page.
    fn load_stories_next_page(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // If the current stories list can accomodate another view page, do nothing. Otherwise,
        // load new stories.

        if self.stories.len() < self.pos + self.page_size - (self.pos % self.page_size) {
            self.load_stories_including(self.pos + STORIES_PER_SITE_PAGE - 1)?;
        }

        Ok(())
    }

    fn paginate(&mut self) -> Vec<(bool, Story)> {
        let mut chunks = self.stories.chunks(self.page_size);
        let chunk = chunks.nth(self.view_page()).unwrap();
        chunk
            .iter()
            .enumerate()
            .map(|(idx, story)| (idx == self.pos % self.page_size, story.to_owned()))
            .collect()
    }

    fn go_to(&mut self, travel: Travel) {
        match travel {
            Travel::NextPage => self.pos += self.page_size,
            Travel::PrevPage => self.pos -= self.pos.min(self.page_size),
            Travel::NextStory => self.pos += 1,
            Travel::PrevStory => self.pos -= self.pos.min(1),
            Travel::Top => self.pos = 0,
            Travel::Bottom => self.pos = self.stories.len() - 1,
        }
    }

    fn get_story(&self, pos: usize) -> Option<&Story> {
        self.stories.get(pos)
    }

    fn get_selected_story(&self) -> &Story {
        self.get_story(self.pos).unwrap()
    }

    fn generate_string(&mut self, width: u16) -> String {
        let current_stories_page = self.paginate();
        let displayed_stories = current_stories_page
            .into_iter()
            .map(|(selected, story)| display_story(&story, width - 3, selected));
        displayed_stories.collect::<Vec<String>>().join("\n")
    }

    fn pos(&self) -> usize {
        self.pos
    }

    fn view_page(&self) -> usize {
        self.pos / self.page_size
    }

    /// Returns the number of the site page the story under the position of the view can be found
    /// on.
    ///
    /// ## Note
    ///
    /// The site page number is 1-indexed.
    fn site_page(&self) -> usize {
        self.pos / STORIES_PER_SITE_PAGE + 1
    }
}

enum Travel {
    NextPage,
    PrevPage,
    NextStory,
    PrevStory,
    Top,
    Bottom,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut term = Term::stdout();
    term.set_title("kreeftje");
    term.hide_cursor()?;
    print!("\u{1b}[?1049h"); // Open the alternative screen buffer.
    term.clear_screen()?;

    let (rows, columns) = console::Term::stdout().size();
    let mut view = View::new(rows as usize / 3);

    'listen: loop {
        view.load_stories_next_page()?;
        term.move_cursor_to(0, 0)?;
        term.clear_line()?;

        let s = view.generate_string(columns);
        #[cfg(debug_assertions)]
        term.write_fmt(format_args!(
            "page {} (pos: {})\n",
            view.site_page(),
            view.pos()
        ))?;
        #[cfg(not(debug_assertions))]
        term.write_fmt(format_args!("page {}\n", view.site_page()))?;
        term.write_all(s.as_bytes())?;
        let input = term.read_key()?;

        match input {
            // J ??? vv
            // Load next page.
            Key::Char('J') => view.go_to(Travel::NextPage),
            // K ??? ^^
            // Load previous page.
            Key::Char('K') => view.go_to(Travel::PrevPage),
            // j ??? v
            // Select next story.
            Key::Char('j') | Key::ArrowDown => view.go_to(Travel::NextStory),
            // k ??? ^
            // Select other story.
            Key::Char('k') | Key::ArrowUp => view.go_to(Travel::PrevStory),
            // l ??? >
            // Open comments.
            // Key::Char('l') | Key::ArrowRight => {
            //  todo!();
            // }
            // h ??? <
            // Close comments.
            // Key::Char('h') | Key::ArrowLeft => {
            //  todo!();
            // }
            // g ??? ^^
            // Go to first page.
            Key::Char('g') => view.go_to(Travel::Top),
            // G ??? vv
            // Go to last loaded page.
            Key::Char('G') => view.go_to(Travel::Bottom),
            // Open link in browser.
            Key::Char('o') | Key::Enter => webbrowser::open(view.get_selected_story().url())?,

            // Quit
            Key::Char('q') => {
                // q to quit (<ctrl-C> als works, of course)
                break 'listen;
            }

            // Otherwise, do nothing
            _ => {}
        }

        term.clear_screen()?;
    }

    term.clear_screen()?;
    term.show_cursor()?;
    print!("\u{1b}[?1049l"); // Close the alternative screen buffer again.

    Ok(())
}
