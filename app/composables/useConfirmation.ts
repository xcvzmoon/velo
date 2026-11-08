import { LazyShellConfirmation } from '#components';

type Confirmation = (title: string, description: string) => Promise<boolean>;

/**
 * Opens a Nuxt UI confirmation dialog owned by the current component scope.
 * @returns A function that resolves true only after explicit confirmation. Dismissal,
 * scope disposal, deactivation, and concurrent requests resolve false. Cancel receives focus.
 * @example
 * ```ts
 * const draft = ref<string>('Unapplied edits')
 * const confirm = useConfirmation()
 * if (await confirm('Discard edits?', 'Unapplied changes will be lost.')) {
 *   draft.value = ''
 * }
 * ```
 */
export function useConfirmation(): Confirmation {
  const overlay = useOverlay();
  let dismiss: (() => void) | null = null;

  async function confirm(title: string, description: string): Promise<boolean> {
    if (dismiss) {
      return false;
    }
    const dialog = overlay.create(LazyShellConfirmation, { destroyOnClose: true });
    dismiss = () => {
      dialog.close(false);
    };
    try {
      return (await dialog.open({ title, description }).result) === true;
    } finally {
      dismiss = null;
    }
  }

  onDeactivated(() => {
    dismiss?.();
  });
  onScopeDispose(() => {
    dismiss?.();
  });
  return confirm;
}
