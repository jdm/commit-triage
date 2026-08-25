import { sendMessageToServer, renderAll, getSelectedCommits, confirmBulkAction } from "./main.js";
import { dialogs } from "./dialog-registry.js";
dialogs.push(editorDialog);

editorDialog.addEventListener("close", event => {
    console.log(event);
    renderAll();
});
editorForm.addEventListener("submit", event => {
    console.log(event);
    if (!confirmBulkAction(n => `label ${n} commits?`)) {
        return;
    }
    sendMessageToServer({"SetLabel": [getSelectedCommits(), input.value]});
});

export function openEditorDialog() {
    editorDialog.showModal();
}
