export function blockClass(index: number, complete: ReadonlySet<number>, partial: ReadonlySet<number>) {
  if (complete.has(index)) return "done";
  return partial.has(index) ? "partial" : "";
}
