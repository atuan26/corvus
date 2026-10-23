import { Button, Label, ListBox, ListBoxItem, Popover, Select, SelectValue } from "react-aria-components";
import type { ProviderInfo } from "../api";

interface Props {
  providers: ProviderInfo[];
  value: string | null;
  onChange: (id: string) => void;
}

export function ProviderSelect({ providers, value, onChange }: Props) {
  return (
    <Select
      className="provider-select"
      selectedKey={value}
      onSelectionChange={(key) => key != null && onChange(String(key))}
      isDisabled={providers.length === 0}
    >
      <Label className="sr-only">Translation provider</Label>
      <Button className="select-button">
        <SelectValue />
        <span aria-hidden>▾</span>
      </Button>
      <Popover className="popover">
        <ListBox className="listbox" items={providers}>
          {(p) => (
            <ListBoxItem className="listbox-item" textValue={p.name}>
              <span>{p.name}</span>
              <kbd className="kbd-hint" aria-hidden>
                Alt+{providers.indexOf(p) + 1}
              </kbd>
            </ListBoxItem>
          )}
        </ListBox>
      </Popover>
    </Select>
  );
}
