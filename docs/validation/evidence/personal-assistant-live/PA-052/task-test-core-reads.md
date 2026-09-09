# PA-052 core-read mapping test

Use the active synthetic deceased-account connector to read executor proof,
accounts, and deadlines once each.

Return the verified proof and authority scope. Return all six account records,
including ownership, notice requirement, deadline, closure eligibility, and
the joint-account exclusion. Return both deadline records and their required
document identifiers.

Do not call any write operation or any other endpoint.
