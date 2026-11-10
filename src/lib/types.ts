export type ResourceType = "file" | "folder";

export interface Space {
  id: number;
  name: string;
  createdAt: string;
}

export interface Resource {
  id: number;
  spaceId: number;
  type: ResourceType;
  path: string;
  title: string;
  note: string;
  isMissing: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface MissingReport {
  checked: number;
  markedMissing: number;
  recovered: number;
}
