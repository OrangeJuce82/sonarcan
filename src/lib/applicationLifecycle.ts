export interface PreventableCloseRequest {
  preventDefault(): void;
}

export function handleWindowCloseRequest(
  event: PreventableCloseRequest,
  requestApplicationExit: () => void,
): void {
  event.preventDefault();
  requestApplicationExit();
}

export function projectOpenDialogOptions(title: string): {
  directory: false;
  multiple: false;
  title: string;
  filters: { name: string; extensions: string[] }[];
} {
  return {
    directory: false,
    multiple: false,
    title,
    filters: [{ name: title, extensions: ["sac"] }],
  };
}
