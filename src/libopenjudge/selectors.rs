use scraper::Selector;
use std::sync::LazyLock;

macro_rules! selector {
    ($selector: expr) => {
        scraper::Selector::parse($selector).unwrap()
    };
}

macro_rules! def_lazy_selector {
    ($ident: ident, $selector: expr) => {
        pub static $ident: LazyLock<Selector> = LazyLock::new(|| selector!($selector));
    };
}

// Selectors for page headers like
// <div class="contest-title-tab">
//   <h2><a>CS101</a></h2>
//	 <span>/</span>
//	 <h2>计算思维算法实践</h2>
// </div>
def_lazy_selector!(
    PAGE_HEADER_GROUP_SELECTOR,
    "#header .wrapper .contest-title-tab h2 a"
);
def_lazy_selector!(
    PAGE_HEADER_PROBSET_SELECTOR,
    "#header .wrapper .contest-title-tab h2:nth-child(3)"
);

// Problem details page selectors:
def_lazy_selector!(PROBLEM_PAGE_TITLE_SELECTOR, "#pageTitle h2");
def_lazy_selector!(PROBLEM_PAGE_CONTENT_DTS_SELECTOR, ".problem-content dt");

// Selects users' solutions on problem details page
def_lazy_selector!(PROBLEM_PAGE_SOLUTION_ROW_SELECTOR, ".my-solutions tbody tr");
def_lazy_selector!(ROW_RESULT_SELECTOR, ".result a");
def_lazy_selector!(ROW_TIME_SELECTOR, ".time abbr");

// Submission page Selectors:
def_lazy_selector!(COMPILE_STATUS_SELECTOR, ".compile-status a");
def_lazy_selector!(COMPILER_INFO_SELECTOR, ".submitStatus pre");
def_lazy_selector!(SUBMISSION_DETAILS_DTS_SELECTOR, ".compile-info dt");
def_lazy_selector!(SUBMISSION_CODE_SELECTOR, "#pagebody .wrapper pre");

// Selects a row in problem list of search results
def_lazy_selector!(PROBLEM_LIST_ROW, "#main .problems-list tbody tr");
// Selects inside a row in problem list
def_lazy_selector!(ROW_TITLE_SELECTOR, ".problem-title a");
def_lazy_selector!(ROW_NUMBER_SELECTOR, ".problem-number");
def_lazy_selector!(ROW_ACCEPTED_CNT_SELECTOR, ".accepted");
def_lazy_selector!(ROW_SUBMISSION_CNT_SELECTOR, ".submissions");
def_lazy_selector!(ROW_GROUP_SELECTOR, ".source a:nth-of-type(1)");
def_lazy_selector!(ROW_PROBSET_SELECTOR, ".source a:nth-of-type(2)");

// Selects a row in problem list of probset details
def_lazy_selector!(PROBSET_PROBLEM_ROW, "#main #problemsList tbody tr");
// Selects inside a row in problem list
def_lazy_selector!(PROBSET_ROW_TITLE_SELECTOR, ".title a");
def_lazy_selector!(PROBSET_ROW_NUMBER_SELECTOR, ".problem-id");
def_lazy_selector!(PROBSET_ROW_ACCEPTED_CNT_SELECTOR, ".accepted a");
def_lazy_selector!(PROBSET_ROW_SUBMISSION_CNT_SELECTOR, ".submissions a");
def_lazy_selector!(PROBSET_ROW_SOLVED_TD_SELECTOR, ".solved");
def_lazy_selector!(PROBSET_ROW_SOLVED_IMG_SELECTOR, "img");

// Selects user home page anchor on http://openjudge.cn/
def_lazy_selector!(USER_HOMEPAGE_SELECTOR, "#userMenu li:nth-of-type(2) a");

// Selects on user home page
def_lazy_selector!(
    USERHOME_NAME_SELECTOR,
    ".user-info .owner-info dl dd:nth-of-type(1)"
);
def_lazy_selector!(
    USERHOME_SEX_SELECTOR,
    ".user-info .owner-info dl dd:nth-of-type(2)"
);
def_lazy_selector!(
    USERHOME_SCHOOL_SELECTOR,
    ".user-info .owner-info dl dd:nth-of-type(3)"
);
def_lazy_selector!(
    USERHOME_REGISTER_TIME_SELECTOR,
    ".user-info .owner-info dl dd:nth-of-type(4)"
);

// Selects on group page
def_lazy_selector!(
    GROUP_PAGE_PROBSET_ANCHORS_SELECTOR,
    ".current-contest .practice-info h3 a"
);
def_lazy_selector!(GROUP_PAGE_NAME_SELECTOR, ".group-name h1");
def_lazy_selector!(GROUP_PAGE_DESCRIPTION_SELECTOR, ".group-description");

// Selectors for page bars on problem list page
def_lazy_selector!(PAGEBAR_LAST_SELECTOR, ".page-bar .pages *:last-child");
def_lazy_selector!(PAGEBAR_CURRENT_SELECTOR, ".page-bar .pages .current");
