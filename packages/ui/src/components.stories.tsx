import type { Meta, StoryObj } from '@storybook/react-vite';
import { expect, fn, userEvent, within } from 'storybook/test';
import {
  Button,
  ButtonAnchor,
  Card,
  CardAnchor,
  NavigationAnchor,
  Field,
  Select,
  Notice,
  Loading,
  ErrorState,
  Badge,
  PageHeading,
  Logo,
  Plant,
  ThemeSwitcher,
} from './index';
const meta = {
  title: 'Design system/Controls',
  component: Button,
  tags: ['autodocs'],
  args: { children: 'Continue', onClick: fn() },
} satisfies Meta<typeof Button>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Primary: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole('button', { name: 'Continue' }));
    await expect(args.onClick).toHaveBeenCalled();
  },
};
export const Secondary: Story = { args: { variant: 'secondary' } };
export const Quiet: Story = { args: { variant: 'quiet' } };
export const Pending: Story = {
  args: { pending: true },
  play: async ({ canvasElement }) => {
    const button = within(canvasElement).getByRole('button');
    await expect(button).toBeDisabled();
    await expect(button).toHaveAttribute('aria-busy', 'true');
  },
};
export const Disabled: Story = { args: { disabled: true } };
export const Fields: Story = {
  render: () => (
    <div className="max-w-sm space-y-5">
      <Field
        label="Email"
        type="email"
        name="email"
        hint="Use your work email."
        placeholder="you@example.com"
      />
      <Field
        label="Invalid email"
        defaultValue="invalid"
        aria-invalid="true"
        hint="Enter a valid email address."
      />
      <Field label="Read only" value="hello@appshell.test" readOnly />
      <div>
        <label className="label" htmlFor="role">
          Role
        </label>
        <Select id="role" defaultValue="member">
          <option value="member">Member</option>
          <option value="admin">Admin</option>
        </Select>
      </div>
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const input = canvas.getByLabelText('Email');
    await userEvent.type(input, 'person@example.com');
    await expect(input).toHaveValue('person@example.com');
    await expect(input).toHaveAccessibleDescription('Use your work email.');
    await userEvent.selectOptions(canvas.getByLabelText('Role'), 'admin');
    await expect(canvas.getByLabelText('Role')).toHaveValue('admin');
  },
};
export const Feedback: Story = {
  render: () => (
    <div className="max-w-xl space-y-4">
      <Notice>Check your inbox to continue.</Notice>
      <Notice kind="success">Your changes are saved.</Notice>
      <Notice kind="error">We couldn’t save those changes.</Notice>
      <Badge>Admin</Badge>
      <ErrorState message="The server is unavailable." retry={fn()} />
    </div>
  ),
};
export const LoadingState: Story = { render: () => <Loading /> };
export const Surfaces: Story = {
  render: () => (
    <div className="space-y-5">
      <PageHeading
        eyebrow="Workspace"
        title="Space to build"
        description="Shared components, one source of truth."
        action={<Button>Create workspace</Button>}
      />
      <Card className="max-w-lg space-y-4 p-7">
        <Logo name="AppShell" />
        <p>A reusable surface for your application.</p>
        <ButtonAnchor href="#example">Open workspace</ButtonAnchor>
      </Card>
      <CardAnchor href="#example" className="block max-w-lg p-6">
        An interactive card
      </CardAnchor>
      <NavigationAnchor href="#example" data-status="active">
        Overview
      </NavigationAnchor>
      <Plant />
    </div>
  ),
};
export const Appearance: Story = { render: () => <ThemeSwitcher /> };
