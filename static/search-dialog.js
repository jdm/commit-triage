import { goToCommit } from "./main.js";
import { commitExt } from "./commit-registry.js";
import { dialogs } from "./dialog-registry.js";
dialogs.push(searchDialog);

// used by event handler attributes in index.html
window.updateSearch = updateSearch;
window.renderSearchDialog = renderSearchDialog;

export let commits = null;
export let filteredCommits = null;
let searchFirstTime = true;

export function openSearchDialog() {
    searchDialog.showModal();
    if (searchFirstTime) {
        searchFirstTime = false;
        updateSearch();
    }
}
export function traverseSearchResults(delta, filtered = true) {
    const whichCommits = filtered ? filteredCommits : commits;
    const oldIndex = whichCommits.map(commit => commit.hash_number).indexOf(commitExt.hash_number);
    const newIndex = oldIndex >= 0 ? (oldIndex + delta + whichCommits.length) % whichCommits.length : 0;
    goToCommit(whichCommits[newIndex].hash_number.slice(1));
}
async function updateSearch() {
    const response = await fetch("/commits");
    commits = await response.json();
    console.log(commits);
    renderSearchDialog();
}
function renderSearchDialog() {
    const filterFn = searchFilter.value.length > 0
        ? eval(`(commit, text, textLower, hints) => ${searchFilter.value}`)
        : () => true;
    const text = commit => [titleWithAbbreviatedHints(commit), ...commit.body].join("\n");
    filteredCommits = commits.filter(commit => filterFn(
        commit,
        text(commit),
        text(commit).toLowerCase(),
        abbreviatedHints(commit),
    ));
    // only clear the <pre> if the filtering ran without throwing.
    const pre = searchDialog.querySelector("pre");
    pre.innerHTML = `${filteredCommits.length}/${commits.length} commits:\n`;
    for (const commit of filteredCommits) {
        const a = document.createElement("a");
        a.addEventListener("click", event => {
            event.preventDefault();
            goToCommit(commit.hash_number.slice(1));
            searchDialog.close();
        });
        a.textContent = commit.hash_number;
        a.href = `#`;
        pre.append(a, ` - ${titleWithAbbreviatedHints(commit)}\n`);
    }

    function titleWithAbbreviatedHints(commit) {
        return `${abbreviatedHints(commit)}${commit.title}`;
    }
    function abbreviatedHints(commit) {
        let result = "";
        if (commit.hints.some(hint => hint.includes("/!\\ contains changes to WPT expectations!"))) {
            result += "[wpt] ";
        }
        if (commit.hints.some(hint => hint.includes("/!\\ contains libservo or embedder_traits changes!"))) {
            result += "[lib] ";
        }
        if (commit.hints.some(hint => hint.includes("/!\\ contains servoshell changes!"))) {
            result += "[shell] ";
        }
        if (commit.hints.some(hint => hint.includes("/!\\ contains WebIDL changes!"))) {
            result += "[web] ";
        }
        if (commit.hints.includes("/!\\ may contain changes to EXPERIMENTAL_PREFS")) {
            result += "[exp] ";
        }
        if (commit.hints.includes("/!\\ may contain changes to feature flags")) {
            result += "[flag] ";
        }
        return result;
    }
}
