import { Camera, SlidersHorizontal, UserCircle } from "@phosphor-icons/react";
import { useAuth } from "../../AuthGate";
import { ErrorBanner } from "../../components/ui/ErrorBanner";
import { SectionHead } from "../../components/ui/SectionHead";
import { Surface } from "../../components/ui/Surface";
import { Tag } from "../../components/ui/Tag";
import { useSettingsForm } from "../../useUserSettings";
import {
  PlaceholderToggle,
  SettingsMasonry,
  SettingsMeta,
  SettingsPageHead,
  SettingsProfileRow,
  SettingsSoonBtn,
} from "./settingsUi";

export function GeneralSettingsPage() {
  const { me } = useAuth();
  const { settings, error, saving, persist } = useSettingsForm(250);

  return (
    <>
      <SettingsPageHead
        title="Settings"
        subtitle={`Signed in as ${me.username}. Profile and preferences for this account.`}
        action={
          saving ? <Tag>Saving…</Tag> : settings ? <Tag tone="ok">Synced</Tag> : <Tag>Loading…</Tag>
        }
      />

      <ErrorBanner>{error}</ErrorBanner>

      <SettingsMasonry>
        <Surface>
          <SectionHead icon={<UserCircle size={16} weight="duotone" />} title="Profile" />
          <SettingsProfileRow
            avatar={
              settings?.show_avatar !== false ? (
                <Camera size={22} weight="duotone" />
              ) : (
                <UserCircle size={28} weight="duotone" />
              )
            }
            title={me.username}
            subtitle="Photo upload soon"
          />
          <div className="settings-stack settings-divider">
            <PlaceholderToggle
              checked={settings?.show_avatar ?? true}
              onChange={(show_avatar) => persist({ show_avatar })}
              label="Show profile picture"
              hint="Saved to this account"
              disabled={!settings}
            />
            <SettingsSoonBtn>Choose photo… (soon)</SettingsSoonBtn>
          </div>
        </Surface>

        <Surface>
          <SectionHead
            icon={<SlidersHorizontal size={16} weight="duotone" />}
            title="Preferences"
          />
          <div className="settings-stack">
            <PlaceholderToggle
              checked={settings?.compact_sidebar ?? false}
              onChange={(compact_sidebar) => persist({ compact_sidebar })}
              label="Prefer compact sidebar"
              hint="Applies on next visit"
              disabled={!settings}
            />
            <PlaceholderToggle
              checked={settings?.confirm_deletes ?? true}
              onChange={(confirm_deletes) => persist({ confirm_deletes })}
              label="Confirm before deleting"
              hint="Goals and events"
              disabled={!settings}
            />
          </div>
          <SettingsMeta className="mt-2 block">Account id {me.id.slice(0, 8)}…</SettingsMeta>
        </Surface>
      </SettingsMasonry>
    </>
  );
}
