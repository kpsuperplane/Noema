import { useMutation, useQuery } from "@apollo/client/react";
import {
  PrivacySettingsDocument,
  SaveActionReviewerPreferenceDocument,
  type PrivacySettingsQuery,
  type SaveActionReviewerPreferenceMutation,
  type SaveActionReviewerPreferenceMutationVariables
} from "@/generated/graphql";
import { PrivacySettingsPaneContent } from "./PrivacySettingsPaneContent";

export { PrivacySettingsPaneContent } from "./PrivacySettingsPaneContent";

export function PrivacySettingsPane() {
  const privacyResult = useQuery<PrivacySettingsQuery>(PrivacySettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [saveActionReviewerPreference, saveResult] = useMutation<
    SaveActionReviewerPreferenceMutation,
    SaveActionReviewerPreferenceMutationVariables
  >(SaveActionReviewerPreferenceDocument, {
    refetchQueries: [{ query: PrivacySettingsDocument }],
    awaitRefetchQueries: true
  });

  return (
    <PrivacySettingsPaneContent
      reviewer={privacyResult.data?.privacySettings.reviewer ?? null}
      loading={privacyResult.loading && !privacyResult.data}
      error={privacyResult.error?.message ?? null}
      saving={saveResult.loading}
      saveError={saveResult.error?.message ?? null}
      onSaveReviewerPreference={(input) =>
        saveActionReviewerPreference({
          variables: {
            input: {
              providerAccountId: input.providerAccountId,
              selectionMode: input.selectionMode,
              modelProfile: input.modelProfile,
              reasoningEffort: input.reasoningEffort ?? null
            }
          }
        })
      }
    />
  );
}
