import { debouncedUpdateGitShow } from "./git-show.js";
import { commitExt, setCommitExt } from "./commit-registry.js";
import { dialogs } from "./dialog-registry.js";
import { openEditorDialog } from "./editor-dialog.js";
import { openWordCloudDialog } from "./word-cloud-dialog.js";
import { openGotoDialog } from "./goto-dialog.js";
import { commits, openSearchDialog, traverseSearchResults } from "./search-dialog.js";

// used by event handler attributes in index.html
window.doAccept = doAccept;
window.doIgnore = doIgnore;
window.doDone = doDone;
window.doLabel = doLabel;
window.doSelect = doSelect;
window.doWordCloud = doWordCloud;
window.doGoToCommit = doGoToCommit;
window.doNext = doNext;
window.doPrevious = doPrevious;
window.doSearch = doSearch;
window.doNextInSearch = doNextInSearch;
window.doPreviousInSearch = doPreviousInSearch;
window.copyCommitReferencesToClipboard = copyCommitReferencesToClipboard;
window.copyCommitSummaryToClipboard = copyCommitSummaryToClipboard;
window.clearSelection = clearSelection;
window.markCommitsDone = markCommitsDone;

export function sendMessageToServer(message) {
    ws.send(JSON.stringify(message));
}
export function goToCommit(number) {
    sendMessageToServer({"GoToCommit": number});
}

const ws = new WebSocket("/ws");
const selectedCommits = new Set;
ws.addEventListener("message", event => {
    console.log(event.data);
    setCommitExt(JSON.parse(event.data));
    updateSelectCommit();
    updateThingsThatDependOnSelectedCommits();
    title.className = commitExt.commit.state;
    title.textContent = commitExt.commit.title;
    meta.textContent = `${commitExt.commit.date} – ${commitExt.commit.authors.join(", ")}`;
    label.textContent = commitExt.commit.label;
    hints.innerHTML = "";
    for (const hint of commitExt.commit.hints) {
        const li = document.createElement("li");
        li.append(hint);
        hints.append(li);
    }
    content.innerHTML = commitExt.rendered_body;
    input.value = commitExt.commit.label;

    // linkify any text that looks like a qualified issue reference. to avoid
    // false positives, only consider direct descendant text nodes of <p>.
    linkify(
        content.querySelectorAll("p"), /([0-9A-Za-z-]+)[/]([^ ]+)#([0-9]+)/gd,
        (linkText, owner, repo, number) => `https://github.com/${owner}/${repo}/issues/${number}`,
    );

    // linkify any text that looks like github issue references. but to avoid
    // false positives, only consider direct descendant text nodes of <p>,
    // plus the commit title line.
    linkify(
        [title, ...content.querySelectorAll("p")], /#[0-9]+/gd,
        linkText => `https://github.com/servo/servo/issues/${linkText.slice(1)}`,
    );

    // linkify any text that looks like github user mentions. but to avoid
    // false positives, only consider direct descendant text nodes of <p>,
    // plus the commit authors line.
    linkify(
        [meta, ...content.querySelectorAll("p")], /@[0-9A-Za-z-]+/gd,
        linkText => `https://github.com/${linkText.slice(1)}`,
    );

    // linkify any text that looks like git commit hashes. but to avoid
    // false positives, only consider direct descendant text nodes of <p>.
    linkify(
        content.querySelectorAll("p"), /\b[0-9a-f]{7,}\b/gd,
        linkText => `https://github.com/servo/servo/commit/${linkText}`,
    );

    // make all links open in a separate tab by default.
    for (const a of document.querySelectorAll(":any-link")) {
        // like `_blank`, but opens in the same tab every time, so you can keep
        // this tool and the links you click in two separate windows.
        // if you want to open in a new tab, you can still use Ctrl+click or
        // middle click. likewise for a new window, you can still Shift+click.
        a.target = "anotherWindow";
    }

    debouncedUpdateGitShow(commitExt.git_show);
});
addEventListener("keydown", event => {
    console.log(event);
    if (editorDialog.open || gotoDialog.open || searchDialog.open || wordCloudDialog.open) {
        return;
    }
    if (event.ctrlKey && event.key == "c") {
        // only if no text is selected.
        // if text is selected, let the browser copy as usual.
        if (getSelection().isCollapsed) {
            event.preventDefault();
            copyCommitReferencesToClipboard();
        }
    }
});
addEventListener("blur", event => {
    console.log(event);
    selectButton.disabled = false;
}, true);
addEventListener("focus", event => {
    console.log(event);
    if (["INPUT", "BUTTON"].includes(event.target.nodeName)) {
        if (event.target != selectButton) {
            selectButton.disabled = true;
        }
    }
}, true);
addEventListener("keypress", event => {
    console.log(event);
    if (dialogs.some(dialog => dialog.open)) {
        return;
    }
    switch (event.key) {
    case "q":
        // require the user to focus the commit-triage TUI to quit.
        // even though this page is in focus, you may still be able to scroll
        // through `git show` in a nearby terminal. if you then press `Q` to
        // quit the `git show` pager without focusing the terminal, you would
        // quit the commit-triage tool by mistake. so we prevent that.
        break;
    case "+":
        doAccept();
        break;
    case "-":
        doIgnore();
        break;
    case ".":
        doDone();
        break;
    case "t":
        doLabel();
        break;
    case " ":
        // if a checkbox or button is already focused,
        // let the browser toggle the checkbox or click the button as usual.
        // otherwise let’s toggle that checkbox here.
        if (!["INPUT", "BUTTON"].includes(event.target.nodeName)) {
            // suppress scroll when you press Space
            event.preventDefault();
            doSelect();
        }
        break;
    case "w":
        doWordCloud();
        break;
    case "g":
        doGoToCommit();
        break;
    case "j":
        doNext();
        break;
    case "k":
        doPrevious();
        break;
    case "/":
        // suppress firefox “quick find”
        event.preventDefault();
        doSearch();
        break;
    case "J":
        doNextInSearch();
        break;
    case "K":
        doPreviousInSearch();
        break;
    default:
        sendMessageToServer({"Keypress": event.key});
    }
});
// when you Shift+scroll, step through commits.
// unlike Ctrl+scroll, Alt+scroll, and scrolling without modifiers, this does
// not seem to interfere with typical browser default behaviour.
addEventListener("wheel", event => {
    console.log(event);
    if (event.target.id == "gitShow") {
        return;
    }
    if (!event.shiftKey || event.altKey || event.ctrlKey || event.metaKey) {
        return;
    }
    if (event.deltaY > 0) {
        doNextInSearch();
        return;
    }
    if (event.deltaY < 0) {
        doPreviousInSearch();
        return;
    }
});
openSearchDialog();

function doAccept() {
    sendMessageToServer({"Keypress": "+"});
    doNextInSearch();
}
function doIgnore() {
    sendMessageToServer({"Keypress": "-"});
    doNextInSearch();
}
function doDone() {
    sendMessageToServer({"Keypress": "."});
    doNextInSearch();
}
function doLabel() {
    openEditorDialog();
}
function doSelect() {
    selectCommit.checked = !selectCommit.checked;
    selectCommitChanged();
}
function doWordCloud() {
    openWordCloudDialog();
}
function doGoToCommit() {
    openGotoDialog();
}
function doNext() {
    sendMessageToServer({"Keypress": "j"});
}
function doPrevious() {
    sendMessageToServer({"Keypress": "k"});
}
function doSearch() {
    openSearchDialog();
}
function doNextInSearch() {
    traverseSearchResults(+1);
}
function doPreviousInSearch() {
    traverseSearchResults(-1);
}
function getCommit(number) {
    return commits.find(commit => commit.hash_number == number);
}
function linkify(parents, regex, hrefFn) {
    for (const parent of parents) {
        for (const kid of parent.childNodes) {
            if (kid.nodeName != "#text")
                continue;
            const originalText = kid.nodeValue;
            // array of matches
            // where each match is an array [matchedText, ...captureGroups]
            // where each element is a pair of indices [start, stop]
            const matches = [];
            let result;
            while ((result = regex.exec(originalText)) != null) {
                matches.push(result.indices);
            }
            matches.reverse();
            for (const match of matches) {
                const [matchedText, ...captureGroups] = match;
                const [start, stop] = matchedText;
                kid.splitText(stop);
                const text = kid.splitText(start);
                const a = document.createElement("a");
                a.href = hrefFn(...match.map(([start, stop]) => originalText.slice(start, stop)));
                text.replaceWith(a);
                a.append(text);
            }
        }
    }
}
function copyCommitReferencesToClipboard() {
    const commits = getSelectedCommits();
    const authors = new Set(commits.map(commit => commit.authors).flat());
    const numbers = commits.map(commit => commit.hash_number);
    const text = `(${[...authors].join(", ")}, ${numbers.join(", ")})`;
    copyTextToClipboard(text);
}
function copyCommitSummaryToClipboard() {
    let text = "";
    for (const commit of getSelectedCommits()) {
        text += `(${commit.authors.join(", ")}, ${commit.hash_number})  ${commit.title}\n`;
        text += `${commit.label}\n\n`;
    }
    copyTextToClipboard(text);
}
function copyTextToClipboard(text) {
    navigator.clipboard.writeText(text);
    alert(`copied to clipboard:\n${text}`);
}

selectCommit.addEventListener("change", event => {
    console.log(event);
    selectCommitChanged();
});
function selectCommitChanged() {
    if (selectCommit.checked) {
        selectedCommits.add(commitExt.commit.hash_number);
    } else {
        selectedCommits.delete(commitExt.commit.hash_number);
    }
    updateThingsThatDependOnSelectedCommits();
}
function updateSelectCommit() {
    selectCommit.checked = selectedCommits.has(commitExt.commit.hash_number);
}
function updateThingsThatDependOnSelectedCommits() {
    copyReferenceButton.disabled = (selectedCommits.size > 0);
    ifAnySelectedCommits.hidden = (selectedCommits.size == 0);
    selectedCommitCount.textContent = `(${selectedCommits.size})`;
}
function clearSelection() {
    selectedCommits.clear();
    updateSelectCommit();
    updateThingsThatDependOnSelectedCommits();
}
function markCommitsDone() {
    ws.send(JSON.stringify({"SetState": [getSelectedCommits(), "Done"]}));
}
function getSelectedCommits() {
    if (selectedCommits.size > 0) {
        return [...selectedCommits].map(number => getCommit(number));
    } else {
        return [commitExt.commit];
    }
}
