import { debouncedUpdateGitShow } from "./git-show.js";

// used by event handler attributes in index.html
window.updateSearch = updateSearch;
window.renderSearchDialog = renderSearchDialog;
window.updateWordCloud = updateWordCloud;

const ws = new WebSocket("/ws");
let commit = null, commits = null, filteredCommits = null;
let searchFirstTime = true;
let wordCloudFirstTime = true;
ws.addEventListener("message", event => {
    console.log(event.data);
    commit = JSON.parse(event.data);
    title.className = commit.commit.state;
    title.textContent = commit.commit.title;
    meta.textContent = `${commit.commit.date} – ${commit.commit.authors.join(", ")}`;
    label.textContent = commit.commit.label;
    hints.innerHTML = "";
    for (const hint of commit.commit.hints) {
        const li = document.createElement("li");
        li.append(hint);
        hints.append(li);
    }
    content.innerHTML = commit.rendered_body;
    input.value = commit.commit.label;

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

    debouncedUpdateGitShow(commit.git_show);
});
addEventListener("keypress", event => {
    console.log(event);
    if (editorDialog.open || gotoDialog.open || searchDialog.open || wordCloudDialog.open) {
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
    case "-":
    case "+":
    case ".":
        ws.send(JSON.stringify({"Keypress": event.key}));
        traverseCommits(+1);
        break;
    case "J":
        traverseCommits(+1);
        break;
    case "K":
        traverseCommits(-1);
        break;
    case "t":
        editorDialog.showModal();
        break;
    case "g":
        gotoDialog.showModal();
        gotoInput.value = commit.commit.hash_number.slice(1);
        gotoInput.select();
        break;
    case "/":
        // suppress firefox “quick find”
        event.preventDefault();
        openSearchDialog();
        break;
    case "w":
        wordCloudDialog.showModal();
        if (wordCloudFirstTime) {
            wordCloudFirstTime = false;
            updateWordCloud();
        }
        break;
    default:
        ws.send(JSON.stringify({"Keypress": event.key}));
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
        traverseCommits(+1);
        return;
    }
    if (event.deltaY < 0) {
        traverseCommits(-1);
        return;
    }
});
editorDialog.addEventListener("close", event => {
    console.log(event);
    ws.send(JSON.stringify({"Reload": null}));
});
editorForm.addEventListener("submit", event => {
    console.log(event);
    ws.send(JSON.stringify({"SetLabel": input.value}));
});
gotoDialog.addEventListener("close", event => {
    console.log(event);
    ws.send(JSON.stringify({"Reload": null}));
});
gotoForm.addEventListener("submit", event => {
    console.log(event);
    goToCommit(gotoInput.value);
});
openSearchDialog();

function goToCommit(number) {
    ws.send(JSON.stringify({"GoToCommit": number}));
}
function traverseCommits(delta) {
    const oldIndex = filteredCommits.map(commit => commit.hash_number).indexOf(commit.commit.hash_number);
    const newIndex = oldIndex >= 0 ? (oldIndex + delta + filteredCommits.length) % filteredCommits.length : 0;
    goToCommit(filteredCommits[newIndex].hash_number.slice(1));
}
async function updateSearch() {
    const response = await fetch("/commits");
    commits = await response.json();
    console.log(commits);
    renderSearchDialog();
}
function openSearchDialog() {
    searchDialog.showModal();
    if (searchFirstTime) {
        searchFirstTime = false;
        updateSearch();
    }
}
function renderSearchDialog() {
    const filterFn = searchFilter.value.length > 0
        ? eval(`(commit, text) => ${searchFilter.value}`)
        : (_commits, _text) => true;
    filteredCommits = commits.filter(commit => filterFn(commit, [titleWithAbbreviatedHints(commit), ...commit.body].join("\n")));
    // only clear the <pre> if the filtering ran without throwing.
    const pre = searchDialog.querySelector("pre");
    pre.innerHTML = `${filteredCommits.length}/${commits.length} commits:\n`;
    for (const commit of filteredCommits) {
        const a = document.createElement("a");
        a.addEventListener("click", event => {
            event.preventDefault();
            goToCommit(commit.hash_number.slice(1));
            wordCloudDialog.close();
        });
        a.textContent = commit.hash_number;
        a.href = `#`;
        pre.append(a, ` - ${titleWithAbbreviatedHints(commit)}\n`);
    }

    function titleWithAbbreviatedHints(commit) {
        let result = "";
        if (commit.hints.some(hint => hint.includes("/!\\ contains changes to WPT expectations!"))) {
            result += "[wpt] ";
        }
        if (commit.hints.some(hint => hint.includes("/!\\ contains libservo changes!"))) {
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
        return result + commit.title;
    }
}
async function updateWordCloud() {
    const response = await fetch("/wordCloud");
    const json = await response.json();
    console.log(json);
    const pre = wordCloudDialog.querySelector("pre");
    pre.innerHTML = "";
    if ("Ok" in json) {
        for (const [word, entries] of json.Ok.words) {
            pre.append(`[${entries.length}] ${word}\n`);
            for (const entry of entries) {
                const a = document.createElement("a");
                a.addEventListener("click", event => {
                    event.preventDefault();
                    goToCommit(entry.hash_number.slice(1));
                    wordCloudDialog.close();
                });
                a.textContent = entry.hash_number;
                a.href = `#`;
                pre.append(a, ` - ${entry.title}\n`);
            }
        }
    } else if ("Err" in json) {
        pre.append(`>>> error: ${json.Err}`);
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
