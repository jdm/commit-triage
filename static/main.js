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
window.doReselect = doReselect;
window.doWordCloud = doWordCloud;
window.doGoToCommit = doGoToCommit;
window.doNext = doNext;
window.doPrevious = doPrevious;
window.doSearch = doSearch;
window.doNextInSearch = doNextInSearch;
window.doPreviousInSearch = doPreviousInSearch;
window.doSearchApiDocs = doSearchApiDocs;
window.doCopyStrong = doCopyStrong;
window.doCopyStrongSingleQuoted = doCopyStrongSingleQuoted;
window.doCopyCode = doCopyCode;
window.doCreateLink = doCreateLink;
window.doCreateCodeLink = doCreateCodeLink;
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

    // inject soft hyphens into text that looks like camelCase, UpperCamelCase,
    // or snake_case. but to avoid false positives, only consider direct
    // descendant text nodes of <p> and <code>, plus the commit title line.
    softHyphenify([title, label, ...content.querySelectorAll("p, code")]);

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
    case "Enter":
        // don’t forward Enter to the TUI, because it will open the label editor,
        // which will eat keys like `+`, `-`, `j`, and `k`.
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
        doNextInSearch();
        break;
    case "k":
        doPreviousInSearch();
        break;
    case "/":
        // suppress firefox “quick find”
        event.preventDefault();
        doSearch();
        break;
    case "J":
        doNext();
        break;
    case "K":
        doPrevious();
        break;
    default:
        sendMessageToServer({"Keypress": event.key});
    }
});
// when you Shift+scroll, step through commits.
// typical browser default behaviour is to scroll horizontally, so ignore the event
// and let that happen if a dialog is open or the pointer is over `#gitShow`.
addEventListener("wheel", event => {
    console.log(event);
    if (event.target.id == "commitScrollArea") {
        event.preventDefault();
        if (event.deltaY > 0) {
            doNextInSearch();
            return;
        }
        if (event.deltaY < 0) {
            doPreviousInSearch();
            return;
        }
    }
}, {passive: false});
openSearchDialog();

function doAccept() {
    if (!confirmBulkAction(n => `mark ${n} commits Accepted?`)) {
        return;
    }
    markCommitsAccepted();
    if (!commitSelectionIsActive()) {
        doNextInSearch();
    }
}
function doIgnore() {
    if (!confirmBulkAction(n => `mark ${n} commits Ignored?`)) {
        return;
    }
    markCommitsIgnored();
    if (!commitSelectionIsActive()) {
        doNextInSearch();
    }
}
function doDone() {
    if (!confirmBulkAction(n => `mark ${n} commits Done?`)) {
        return;
    }
    markCommitsDone();
    if (!commitSelectionIsActive()) {
        doNextInSearch();
    }
}
function doLabel() {
    openEditorDialog();
}
function doSelect() {
    selectCommit.checked = !selectCommit.checked;
    selectCommitChanged();
}
function doReselect() {
    const input = prompt("select commit numbers:", "(@handle, @handle, #number, #number)");
    for (const hash_number of input.match(/#[0-9]+/g) ?? []) {
        if (getCommit(hash_number) != null) {
            selectedCommits.add(hash_number);
        }
    }
    updateSelectCommit();
    updateThingsThatDependOnSelectedCommits();
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
function doSearchApiDocs() {
    // strip out any soft hyphens from the query, because they break the search.
    const query = `${getSelection()}`.replace(/\u00AD/g, "");
    // like `_blank`, but opens in the same tab every time, so you can keep
    // this tool and the API docs in two separate windows.
    open(`https://doc.servo.org/servo/?search=${query}`, "anotherWindow");
}
function doCopyStrong() {
    const escapedSelection = escapeForCopyingMarkdown(`${getSelection()}`);
    copyTextToClipboard(`**${escapedSelection}**`);
}
function doCopyStrongSingleQuoted() {
    const escapedSelection = escapeForCopyingMarkdown(`${getSelection()}`);
    copyTextToClipboard(`**‘${escapedSelection}’**`);
}
function doCopyCode() {
    const escapedSelection = escapeForCopyingMarkdown(`${getSelection()}`);
    copyTextToClipboard(`\`${escapedSelection}\``);
}
async function doCreateLink() {
    const escapedSelection = escapeForCopyingMarkdown(`${getSelection()}`);
    const url = await navigator.clipboard.readText();
    copyTextToClipboard(`[${escapedSelection}](${url})`);
}
async function doCreateCodeLink() {
    const escapedSelection = escapeForCopyingMarkdown(`${getSelection()}`);
    const url = await navigator.clipboard.readText();
    copyTextToClipboard(`[\`${escapedSelection}\`](${url})`);
}
function escapeForCopyingMarkdown(text) {
    return text.replace(/&/g, "&amp;").replace(/</g, "&lt;");
}
function getCommit(number) {
    return commits.find(commit => commit.hash_number == number);
}
function markCommitsAccepted() {
    ws.send(JSON.stringify({"SetState": [getSelectedCommits(), "Accepted"]}));
}
function markCommitsIgnored() {
    ws.send(JSON.stringify({"SetState": [getSelectedCommits(), "Ignored"]}));
}
function markCommitsDone() {
    ws.send(JSON.stringify({"SetState": [getSelectedCommits(), "Done"]}));
}
function softHyphenify(parents) {
    if (softHyphens.elements.value.value == "off") {
        return;
    }
    for (const parent of parents) {
        for (const kid of parent.childNodes) {
            if (kid.nodeName != "#text")
                continue;
            const replacement = softHyphens.elements.value.value == "debug"
                ? "$1-$2"
                : "$1\u00AD$2";
            // FIXME: injects extra hyphen in cases like `innerHTML` → `inner-HTM-L`
            kid.nodeValue = kid.nodeValue.replace(/([a-z]|[A-Z]+)([A-Z]|_)/g, replacement);
        }
    }
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
    const text = linkifyCopies.elements.value.value == "on"
        ? `(${[...authors].map(a => `[${a}](https://github.com/${a.slice(1)})`).join(", ")}, ${numbers.map(n => `[${n}](https://github.com/servo/servo/pull/${n.slice(1)})`).join(", ")})`
        : `(${[...authors].join(", ")}, ${numbers.join(", ")})`;
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
    acceptButton.disabled = (selectedCommits.size > 0);
    ignoreButton.disabled = (selectedCommits.size > 0);
    doneButton.disabled = (selectedCommits.size > 0);
    labelButton.disabled = (selectedCommits.size > 0);
    copyReferenceButton.disabled = (selectedCommits.size > 0);
    ifAnySelectedCommits.hidden = (selectedCommits.size == 0);
    selectedCommitCount.textContent = `(${selectedCommits.size})`;
}
function clearSelection() {
    selectedCommits.clear();
    updateSelectCommit();
    updateThingsThatDependOnSelectedCommits();
}
export function getSelectedCommits() {
    if (commitSelectionIsActive()) {
        return [...selectedCommits].map(number => getCommit(number));
    } else {
        return [commitExt.commit];
    }
}
export function commitSelectionIsActive() {
    return selectedCommits.size > 0;
}
/// usage: ``if (!confirmBulkAction(n => `do something to ${n} commits?`)) return;``
export function confirmBulkAction(messageFn) {
    return !commitSelectionIsActive() || confirm(messageFn(getSelectedCommits().length));
}

softHyphens.addEventListener("change", event => {
    console.log(event);
    sendMessageToServer({"Reload": null});
}, true);
document.addEventListener("selectionchange", event => {
    console.log(event);
    const selection = getSelection();
    if (selection.isCollapsed) {
        selectionToolbox.hidden = true;
    } else {
        selectionToolbox.hidden = false;
        const range = selection.getRangeAt(0);
        const rects = [...range.getClientRects()];
        // getClientRects() on Range is guaranteed to return the rects in content order,
        // and i’ve never seen it merge lines, so `.at(-1)` gives us the last line’s rect.
        const lastLineRect = rects.at(-1);
        // the selection toolbox hangs down and to the left of the selection’s bottom right corner,
        // but we don’t want it to go past the left edge or the bottom edge of the viewport.
        // define the minimum `x` and the maximum `y` of the selection’s bottom right corner,
        // beyond which we’ll do something special.
        const minX = selectionToolboxContent.offsetWidth;
        const maxY = innerHeight - selectionToolboxContent.offsetHeight;
        // if `x` < `minX`, clamp the selection toolbox to the left edge of the viewport.
        // if `y` > `maxY`, have the selection toolbox sit above the last line of text.
        const x = Math.max(lastLineRect.right, minX);
        const y = lastLineRect.bottom <= maxY
            ? lastLineRect.bottom
            : lastLineRect.top - selectionToolboxContent.offsetHeight;
        selectionToolbox.style.left = `${x}px`;
        selectionToolbox.style.top = `${y}px`;
    }
});
// ergonomics fix: avoid interfering with text selection,
// if the pointer moves onto `#selectionToolbox`.
addEventListener("mousedown", event => {
    console.log(event);
    if (event.target.closest("#selectionToolbox") == null) {
        selectionToolbox.inert = true;
    }
});
addEventListener("mouseup", event => {
    console.log(event);
    selectionToolbox.inert = false;
});
