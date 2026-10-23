import type { Ref } from "react";
import { Button, ComboBox, Input, Label, ListBox, ListBoxItem, Popover } from "react-aria-components";
import type { Language } from "../api";

interface Props {
  label: string;
  languages: Language[];
  value: string;
  onChange: (code: string) => void;
  inputRef?: Ref<HTMLInputElement>;
}

export function LanguagePicker({ label, languages, value, onChange, inputRef }: Props) {
  const items = languages.map((l) => ({ id: l.code, name: l.name }));
  return (
    <ComboBox
      className="lang-picker"
      items={items}
      selectedKey={value}
      onSelectionChange={(key) => key != null && onChange(String(key))}
      menuTrigger="focus"
      allowsEmptyCollection
    >
      <Label className="sr-only">{label}</Label>
      <div className="field">
        <Input ref={inputRef} className="field-input" />
        <Button className="field-button" aria-label={`Show ${label.toLowerCase()} list`}>
          <span aria-hidden>▾</span>
        </Button>
      </div>
      <Popover className="popover">
        <ListBox className="listbox" renderEmptyState={() => <div className="listbox-empty">No match</div>}>
          {(item: { id: string; name: string }) => (
            <ListBoxItem className="listbox-item" textValue={item.name}>
              {item.name}
            </ListBoxItem>
          )}
        </ListBox>
      </Popover>
    </ComboBox>
  );
}
