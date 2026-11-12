import { addTodo, toggleTodo } from "../bridge";
import type { OfficeSnapshot, Todo } from "../types";

export class Board {
  private todos: Todo[] = [];
  private focus: string | null = null;
  private path = "";
  private busy = false;

  constructor(private readonly onChange: (snapshot: OfficeSnapshot) => void) {
    document.querySelector<HTMLButtonElement>("#board-close")!.addEventListener("click", () => {
      document.querySelector<HTMLDialogElement>("#board-dialog")!.close();
    });
    document.querySelector<HTMLFormElement>("#board-form")!.addEventListener("submit", (event) => {
      event.preventDefault();
      void this.add();
    });
  }

  set(todos: Todo[], focus: string | null, path: string) {
    this.todos = todos;
    this.focus = focus;
    this.path = path;
    this.render();
  }

  open() {
    this.render();
    const dialog = document.querySelector<HTMLDialogElement>("#board-dialog")!;
    if (!dialog.open) dialog.showModal();
    document.querySelector<HTMLInputElement>("#todo-input")!.focus();
  }

  private render() {
    const focus = document.querySelector<HTMLElement>("#board-focus")!;
    focus.hidden = !this.focus;
    focus.textContent = this.focus ?? "";
    document.querySelector("#todo-path")!.textContent = this.path
      ? `Saved to ${this.path}. Items from the today cache are copied in when you check them.`
      : "";
    const list = document.querySelector<HTMLUListElement>("#todo-list")!;
    list.replaceChildren();
    const empty = document.querySelector<HTMLElement>("#todo-empty")!;
    empty.hidden = this.todos.length > 0;
    for (const item of this.todos) {
      const li = document.createElement("li");
      if (item.done) li.classList.add("done");
      const label = document.createElement("label");
      const box = document.createElement("input");
      box.type = "checkbox";
      box.checked = item.done;
      box.addEventListener("change", () => {
        void this.toggle(item.id);
      });
      const text = document.createElement("span");
      text.textContent = item.text;
      label.append(box, text);
      if (item.source === "today") {
        const tag = document.createElement("span");
        tag.className = "tag";
        tag.textContent = "today";
        label.append(tag);
      }
      li.append(label);
      list.append(li);
    }
  }

  private async add() {
    if (this.busy) return;
    const input = document.querySelector<HTMLInputElement>("#todo-input")!;
    const text = input.value.trim();
    if (!text) return;
    this.busy = true;
    this.error("");
    try {
      const snapshot = await addTodo(text);
      input.value = "";
      this.onChange(snapshot);
    } catch (error) {
      this.error(error instanceof Error ? error.message : "The board did not save.");
    } finally {
      this.busy = false;
    }
  }

  private async toggle(id: string) {
    this.error("");
    try {
      const snapshot = await toggleTodo(id);
      this.onChange(snapshot);
    } catch (error) {
      this.error(error instanceof Error ? error.message : "The board did not save.");
      this.render();
    }
  }

  private error(text: string) {
    const node = document.querySelector<HTMLElement>("#todo-error")!;
    node.hidden = !text;
    node.textContent = text;
  }
}
