import { createContext, useContext } from 'react';
import type { Organization, Session } from './schemas';
export const WorkspaceContext = createContext<{
  session: Session;
  organization: Organization;
} | null>(null);
export function useWorkspace() {
  const value = useContext(WorkspaceContext);
  if (!value) throw new Error('Workspace context is missing');
  return value;
}
