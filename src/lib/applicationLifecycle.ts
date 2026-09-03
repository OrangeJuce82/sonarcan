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

export interface ProjectOpenDialogOptions {
  directory: boolean;
  multiple: false;
  title: string;
  filters?: { name: string; extensions: string[] }[];
}

export function projectOpenDialogOptions(
  title: string,
  projectPackagesAreFiles = true,
): ProjectOpenDialogOptions {
  const options: ProjectOpenDialogOptions = {
    directory: !projectPackagesAreFiles,
    multiple: false,
    title,
  };
  if (projectPackagesAreFiles) options.filters = [{ name: title, extensions: ["sac"] }];
  return options;
}
