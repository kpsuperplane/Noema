import { ErrorMarker } from "../ErrorMarker";

export function ErrorNotice({ message, recoverable }: { message: string; recoverable: boolean }) {
  return <ErrorMarker message={message} label={recoverable ? "Notice" : "Error"} recoverable={recoverable} />;
}
