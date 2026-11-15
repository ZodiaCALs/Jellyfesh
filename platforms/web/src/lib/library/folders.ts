// Reading the person's music folders through the browser, for one visit: Chrome and Edge's folder
// picker (`showDirectoryPicker`) gives a handle the library walks; other browsers hand over the
// files of a folder picked with `<input webkitdirectory>`. Nothing is kept between visits (see
// ./store.ts): the catalogue stays, and the person picks the folder again to play from it.

/** The part of the File System Access API the library uses (not in every TypeScript DOM lib). */
interface IterableDirectory {
  values(): AsyncIterable<FileSystemDirectoryHandle | FileSystemFileHandle>;
}

type DirectoryPicker = (options?: { id?: string; mode?: 'read' }) => Promise<FileSystemDirectoryHandle>;

/** One file of a folder: its path inside the folder (`/`-separated) and the file. */
export interface FolderFile {
  path: string;
  file: File;
}

/** A folder the person picked. */
export interface PickedFolder {
  name: string;
  /** Chrome and Edge's handle, for this visit. */
  handle: FileSystemDirectoryHandle | null;
  /** The files, when the browser gave them instead of a handle. */
  files: FolderFile[] | null;
}

function picker(): DirectoryPicker | null {
  const pick = (globalThis as unknown as { showDirectoryPicker?: DirectoryPicker }).showDirectoryPicker;
  return typeof pick === 'function' ? pick : null;
}

/** The files `<input webkitdirectory>` gave, as paths inside the picked folder. */
export function filesOfInput(list: FileList | File[]): PickedFolder | null {
  const files = [...list];
  if (files.length === 0) return null;
  const first = (files[0] as File & { webkitRelativePath?: string }).webkitRelativePath ?? '';
  const name = first.split('/')[0] || files[0].name;
  return {
    name,
    handle: null,
    files: files.map((file) => {
      const relative = (file as File & { webkitRelativePath?: string }).webkitRelativePath ?? file.name;
      const slash = relative.indexOf('/');
      return { path: slash >= 0 ? relative.slice(slash + 1) : relative, file };
    }),
  };
}

/**
 * Asks the person for a folder: the browser's folder picker where it has one, else `fallback`,
 * which must open an `<input webkitdirectory>` and resolve with what it picked. Null when the
 * person cancelled.
 */
export async function pickFolder(fallback: () => Promise<PickedFolder | null>): Promise<PickedFolder | null> {
  const pick = picker();
  if (!pick) return fallback();
  try {
    const handle = await pick({ id: 'jellyfesh-music', mode: 'read' });
    return { name: handle.name, handle, files: null };
  } catch (error) {
    if ((error as DOMException).name === 'AbortError') return null;
    throw error;
  }
}

/** Every file in a folder and its subfolders. Hidden files and folders are skipped. */
export async function* walk(handle: FileSystemDirectoryHandle, prefix = ''): AsyncGenerator<FolderFile> {
  for await (const entry of (handle as unknown as IterableDirectory).values()) {
    if (entry.name.startsWith('.')) continue;
    const path = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.kind === 'directory') {
      yield* walk(entry as FileSystemDirectoryHandle, path);
    } else {
      try {
        yield { path, file: await (entry as FileSystemFileHandle).getFile() };
      } catch {
        // Gone or unreadable since the listing: skipped.
      }
    }
  }
}

/** The file at `path` inside `handle`, or null when it is not there any more. */
export async function fileAt(handle: FileSystemDirectoryHandle, path: string): Promise<File | null> {
  try {
    const parts = path.split('/');
    let dir = handle;
    for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part);
    return await (await dir.getFileHandle(parts[parts.length - 1])).getFile();
  } catch {
    return null;
  }
}
