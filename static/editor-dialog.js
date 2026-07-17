import { sendMessageToServer } from "./main.js";
import { getSelectedCommits } from "./commit-registry.js";
import { dialogs } from "./dialog-registry.js";
dialogs.push(editorDialog);

editorDialog.addEventListener("close", event => {
    console.log(event);
    sendMessageToServer({"Reload": null});
});
editorForm.addEventListener("submit", event => {
    console.log(event);
    sendMessageToServer({"SetLabel": [getSelectedCommits(), input.value]});
});

export function openEditorDialog() {
    editorDialog.showModal();
}
