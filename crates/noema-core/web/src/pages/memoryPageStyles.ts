import * as stylex from "@stylexjs/stylex";

const wikiSerif = "Georgia, 'Times New Roman', serif";
const wikiSans = "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif";

export const styles = stylex.create({
  surface: {
    boxSizing: "border-box",
    height: "100%",
    minHeight: 0,
    width: "100%",
    display: "flex",
    flexDirection: "column",
  },
  wikiShell: {
    borderStyle: "none",
    borderWidth: 1,
    borderTopStyle: "solid",
    borderColor: "#a2a9b1",
    backgroundColor: "white",
    display: "flex",
    flexDirection: "column",
    height: "100%",
  },
  tabs: {
    display: "flex",
    alignItems: "stretch",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "#a2a9b1",
    backgroundColor: "#f8f9fa",
    fontFamily: wikiSans,
    fontSize: 13,
    overflowX: "auto",
    flexShrink: 0,
    flexGrow: 0,
  },
  tab: {
    borderRightWidth: 1,
    borderRightStyle: "solid",
    borderRightColor: "#a2a9b1",
    color: "#36c",
    padding: "10px 14px",
    whiteSpace: "nowrap"
  },
  tabActive: {
    borderRightWidth: 1,
    borderRightStyle: "solid",
    borderRightColor: "#a2a9b1",
    backgroundColor: "white",
    color: "#202122",
    padding: "10px 14px",
    whiteSpace: "nowrap"
  },
  tabEnd: {
    marginLeft: "auto",
    borderLeftWidth: 1,
    borderLeftStyle: "solid",
    borderLeftColor: "#a2a9b1",
    color: "#36c",
    padding: "10px 14px",
    whiteSpace: "nowrap"
  },
  page: {
    maxWidth: 1220,
    margin: "0 auto",
    display: "block",
    padding: "22px 26px 34px",
    color: "#202122",
    fontFamily: wikiSerif,
    "::after": {
      content: "''",
      display: "block",
      clear: "both"
    },
    "@media (max-width: 760px)": {
      padding: "16px 14px 24px"
    }
  },
  pageShell: {
    overflowY: "auto",
    flex: 1,
    display: "block",
  },
  articleTitle: {
    clear: "both",
    margin: 0,
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "#a2a9b1",
    paddingBottom: 6,
    color: "#202122",
    fontFamily: wikiSerif,
    fontSize: 36,
    fontWeight: 400,
    lineHeight: 1.18,
    letterSpacing: 0,
    "@media (max-width: 760px)": {
      fontSize: 31
    }
  },
  subtitle: {
    marginTop: 6,
    color: "#54595d",
    fontFamily: wikiSans,
    fontSize: 12
  },
  statusBlock: {
    marginTop: 16,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#a2a9b1",
    backgroundColor: "#f8f9fa",
    padding: 12,
    color: "#54595d",
    fontFamily: wikiSans,
    fontSize: 13,
    lineHeight: 1.45
  },
  errorBlock: {
    borderColor: "#b98b9d",
    backgroundColor: "#fbf3f6",
    color: "#5e3142"
  },
  infobox: {
    float: "right",
    width: 292,
    margin: "18px 0 18px 24px",
    fontFamily: wikiSans,
    "@media (max-width: 760px)": {
      float: "none",
      width: "auto",
      margin: "16px 0"
    }
  },
  infoTable: {
    width: "100%",
    borderCollapse: "collapse",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#a2a9b1",
    backgroundColor: "#f8f9fa",
    fontSize: 12
  },
  boxTitle: {
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#a2a9b1",
    backgroundColor: "#d8e4df",
    padding: 7,
    textAlign: "center",
    fontSize: 14
  },
  discCell: {
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#a2a9b1",
    backgroundColor: "white",
    padding: 14,
    textAlign: "center"
  },
  clusterDisc: {
    width: 96,
    height: 96,
    margin: "0 auto",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#a2a9b1",
    borderRadius: "50%",
    backgroundImage:
      "conic-gradient(#8da69a 0 38%, #c2b47c 38% 59%, #b98b9d 59% 82%, #b8bdc5 82% 100%)"
  },
  discLabel: {
    marginTop: 6,
    color: "#54595d",
    fontSize: 11
  },
  boxKey: {
    width: "42%",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#a2a9b1",
    backgroundColor: "#eaecf0",
    padding: 7,
    textAlign: "left",
    verticalAlign: "top"
  },
  boxValue: {
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#a2a9b1",
    padding: 7,
    verticalAlign: "top"
  },
  sidebox: {
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#a2a9b1",
    backgroundColor: "#f8f9fa",
    marginTop: 14,
    padding: 10,
    fontSize: 12,
    lineHeight: 1.45
  },
  actionLinks: {
    display: "grid",
    gap: 5,
    marginTop: 8
  },
  lead: {
    margin: "18px 0 0",
    color: "#202122",
    fontFamily: wikiSerif,
    fontSize: 16,
    lineHeight: 1.62,
    "@media (max-width: 760px)": {
      fontSize: 15
    }
  },
  stubNote: {
    display: "flex",
    alignItems: "flex-start",
    gap: 10,
    marginTop: 14,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#a2a9b1",
    borderLeftWidth: 6,
    borderLeftColor: "#c2b47c",
    backgroundColor: "#f8f9fa",
    padding: "9px 11px",
    color: "#202122",
    fontFamily: wikiSans,
    fontSize: 13,
    lineHeight: 1.45,
    "@media (max-width: 560px)": {
      display: "grid",
      gap: 4
    }
  },
  stubLabel: {
    minWidth: 44,
    color: "#54595d",
    fontWeight: 700,
    textTransform: "uppercase",
    fontSize: 11,
    letterSpacing: 0
  },
  contents: {
    float: "left",
    width: 230,
    margin: "18px 22px 12px 0",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#a2a9b1",
    backgroundColor: "#f8f9fa",
    padding: 10,
    fontFamily: wikiSans,
    fontSize: 13,
    lineHeight: 1.75,
    "@media (max-width: 760px)": {
      float: "none",
      width: "auto",
      margin: "16px 0"
    }
  },
  contentsTitle: {
    marginBottom: 6,
    textAlign: "center",
    fontWeight: 700
  },
  contentsList: {
    margin: 0,
    paddingLeft: 22
  },
  link: {
    color: "#36c",
    textDecoration: "none"
  },
  bodyText: {
    color: "#202122",
    fontFamily: wikiSerif,
    fontSize: 15,
    lineHeight: 1.62
  },
  articleSection: {
    marginTop: 26
  },
  sectionTitle: {
    margin: "0 0 8px",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "#a2a9b1",
    paddingBottom: 3,
    color: "#202122",
    fontFamily: wikiSerif,
    fontSize: 24,
    fontWeight: 400,
    lineHeight: 1.25,
    overflow: "hidden"
  },
  entryList: {
    display: "grid",
    gap: 0,
    margin: 0,
    paddingLeft: 18
  },
  entryItem: {
    paddingBlock: 8
  },
  entryText: {
    margin: 0,
    color: "#202122",
    fontFamily: wikiSerif,
    fontSize: 15,
    lineHeight: 1.62
  },
  references: {
    color: "#202122",
    fontFamily: wikiSans,
    fontSize: 12,
    lineHeight: 1.55
  },
  loadMore: {
    clear: "both",
    marginTop: 18,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#72777d",
    borderRadius: 2,
    backgroundColor: "#f8f9fa",
    padding: "8px 12px",
    color: "#202122",
    fontFamily: wikiSans,
    fontSize: 13,
    fontWeight: 600
  },
  regenerateButton: {
    marginTop: 8,
    width: "100%",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#72777d",
    borderRadius: 2,
    backgroundColor: "#f8f9fa",
    padding: "7px 9px",
    color: "#202122",
    fontFamily: wikiSans,
    fontSize: 12,
    fontWeight: 600,
    textAlign: "center",
    ":disabled": {
      color: "#72777d",
      borderColor: "#c8ccd1"
    }
  }
});
