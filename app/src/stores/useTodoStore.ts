import { create } from "zustand";
import {
  todoComplete,
  todoDelete,
  todoList,
  todoUpsert,
  type Todo,
} from "../lib/lifeApi";

interface TodoState {
  todos: Todo[];
  loading: boolean;
  refresh: () => Promise<void>;
  save: (input: Partial<Todo> & { title: string }) => Promise<void>;
  complete: (id: string) => Promise<void>;
  remove: (id: string) => Promise<void>;
}

export const useTodoStore = create<TodoState>((set, get) => ({
  todos: [],
  loading: false,
  refresh: async () => {
    set({ loading: true });
    try {
      set({ todos: await todoList() });
    } finally {
      set({ loading: false });
    }
  },
  save: async (input) => {
    await todoUpsert(input);
    await get().refresh();
  },
  complete: async (id) => {
    await todoComplete(id);
    await get().refresh();
  },
  remove: async (id) => {
    await todoDelete(id);
    await get().refresh();
  },
}));

export function isOverdue(todo: Todo, today: string) {
  return todo.status !== "completed" && !!todo.deadline && todo.deadline < today;
}
