import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider } from '@tanstack/react-router';
import { queryClient } from './lib/query-client';
import { router } from './router';
import './styles/app.css';
import { initializeI18n } from './lib/i18n';
const root = document.getElementById('root');
if (!root) throw new Error('App root is missing');
void initializeI18n().then(() =>
  createRoot(root).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </StrictMode>,
  ),
);
